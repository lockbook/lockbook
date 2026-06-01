use std::collections::HashSet;

use crate::LocalLb;
use crate::model::clock::get_time;
use crate::model::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::secret_filename::HmacSha256;
use crate::model::tree_like::TreeLike;
use crate::model::{compression_service, symkey, validate};
use hmac::{Mac, NewMac};
use uuid::Uuid;

use super::activity;
use super::events::Actor;

impl LocalLb {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument> {
        let (_, doc) = self.read_document_with_hmac(id, user_activity).await?;
        Ok(doc)
    }

    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        // get info so we can do operations while not holding lock
        let (id, key) = {
            let tx = self.ro_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
            let id = match tree.find(&id)?.file_type() {
                FileType::Document | FileType::Folder => id,
                FileType::Link { target } => target,
            };
            validate::is_document(tree.find(&id)?)?;
            (id, tree.decrypt_key(&id, &self.keychain)?)
        };

        // do the operations
        let (hmac, encrypted) = compress_encrypt_document(&key, content)?;
        let encrypted_size = encrypted.value.len();
        self.docs.insert_pending(id, hmac, &encrypted).await?;

        // commit the result
        {
            let mut tx = self.begin_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata)
                .to_staged(&mut db.local_metadata)
                .to_lazy();
            self.docs.promote_pending(id, hmac).await?;
            tree.overwrite_document_hmac(&id, Some(hmac), Some(encrypted_size), &self.keychain)?;
            tx.end();
        }

        self.events.doc_written(id, Actor::User);
        self.add_doc_event(activity::DocEvent::Write(id, get_time().0))
            .await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        // get info + on-disk bytes so we can decrypt without holding the lock
        let info: Option<(DocumentHmac, AESKey, Option<EncryptedDocument>)> = {
            let tx = self.ro_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

            let file = tree.find(&id)?;
            validate::is_document(file)?;
            let hmac = file.document_hmac().copied();

            if tree.calculate_deleted(&id)? {
                return Err(LbErrKind::FileNonexistent.into());
            }

            match hmac {
                Some(hmac) => {
                    let key = tree.decrypt_key(&id, &self.keychain)?;
                    let local_blob = if self.docs.exists(id, Some(hmac)) {
                        Some(self.docs.get(id, Some(hmac)).await?)
                    } else {
                        None
                    };
                    Some((hmac, key, local_blob))
                }
                None => None,
            }
        };

        // do decrypt + decompress without holding the lock; fetch from the
        // server first if the blob wasn't already local.
        let (hmac, doc) = match info {
            None => (None, vec![]),
            Some((hmac, key, local_blob)) => {
                let encrypted = match local_blob {
                    Some(blob) => blob,
                    // todo: if document not found -- need to trigger a pull
                    None => self.fetch_doc(id, hmac).await?,
                };
                let doc = decrypt_decompress_document(&key, &encrypted)?;
                (Some(hmac), doc)
            }
        };

        if user_activity {
            self.add_doc_event(activity::DocEvent::Read(id, get_time().0))
                .await?;
        }

        Ok((hmac, doc))
    }

    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        // get info so we can do operations while not holding lock
        let (target_id, key) = {
            let tx = self.ro_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
            let file = tree.find(&id)?;
            if file.document_hmac() != old_hmac.as_ref() {
                return Err(LbErrKind::ReReadRequired.into());
            }
            let target_id = match file.file_type() {
                FileType::Document | FileType::Folder => id,
                FileType::Link { target } => target,
            };
            validate::is_document(tree.find(&target_id)?)?;
            (target_id, tree.decrypt_key(&target_id, &self.keychain)?)
        };

        // do the operations
        let (hmac, encrypted) = compress_encrypt_document(&key, &content)?;
        let encrypted_size = encrypted.value.len();
        self.docs
            .insert_pending(target_id, hmac, &encrypted)
            .await?;

        // commit the result
        {
            let mut tx = self.begin_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata)
                .to_staged(&mut db.local_metadata)
                .to_lazy();
            self.docs.promote_pending(target_id, hmac).await?;
            if tree.find(&id)?.document_hmac() != old_hmac.as_ref() {
                return Err(LbErrKind::ReReadRequired.into());
            }
            tree.overwrite_document_hmac(
                &target_id,
                Some(hmac),
                Some(encrypted_size),
                &self.keychain,
            )?;
            tx.end();
        }

        // todo: when workspace isn't the only writer, this arg needs to be exposed
        // this will happen when lb-fs is integrated into an app and shares an lb-rs with ws
        // or it will happen when there are multiple co-operative core processes.
        self.events.doc_written(target_id, Actor::User);
        self.add_doc_event(activity::DocEvent::Write(target_id, get_time().0))
            .await?;

        Ok(hmac)
    }

    pub(crate) async fn cleanup(&self) -> LbResult<()> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let tree = db.base_metadata.stage(&db.local_metadata);

        let base_files = tree.base.all_files()?.into_iter();
        let local_files = tree.staged.all_files()?.into_iter();

        let file_hmacs = base_files
            .chain(local_files)
            .filter_map(|f| f.document_hmac().map(|hmac| (*f.id(), *hmac)))
            .collect::<HashSet<_>>();

        self.docs.retain(file_hmacs).await?;

        drop(tx);

        Ok(())
    }
}

fn compress_encrypt_document(
    key: &AESKey, content: &[u8],
) -> LbResult<(DocumentHmac, EncryptedDocument)> {
    let hmac: DocumentHmac = {
        let mut mac = HmacSha256::new_from_slice(key)
            .map_err(|err| LbErrKind::Unexpected(format!("hmac creation error: {err:?}")))?;
        mac.update(content);
        mac.finalize().into_bytes()
    }
    .into();
    let compressed = compression_service::compress(content)?;
    let encrypted = symkey::encrypt(key, &compressed)?;
    Ok((hmac, encrypted))
}

fn decrypt_decompress_document(
    key: &AESKey, encrypted: &EncryptedDocument,
) -> LbResult<DecryptedDocument> {
    let compressed = symkey::decrypt(key, encrypted)?;
    let doc = compression_service::decompress(&compressed)?;
    Ok(doc)
}
