use crate::io::server::Server;
use std::collections::HashSet;

use crate::Lb;
use crate::model::clock::get_time;
use crate::model::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::filename::DocumentType;
use crate::model::secret_filename::HmacSha256;
use crate::model::tree_like::TreeLike;
use crate::model::{compression_service, symkey, validate};
use hmac::{Mac, NewMac};
use uuid::Uuid;

use super::activity;
use super::events::Actor;

impl<S: Server> Lb<S> {
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
        let pending = self.docs.insert_pending(id, hmac, &encrypted).await?;

        // commit the result
        {
            let mut tx = self.begin_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata)
                .to_staged(&mut db.local_metadata)
                .to_lazy();
            pending.promote().await?;
            tree.overwrite_document_hmac(&id, Some(hmac), Some(encrypted_size), &self.keychain)?;
            tx.end();
        }

        self.events.doc_written(id, Actor::User(None));
        self.add_doc_event(activity::DocEvent::Write(id, get_time().0))
            .await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        // get info + on-disk bytes so we can decrypt without holding the lock
        let mut retried = false;
        let info: Option<(DocumentHmac, AESKey, Option<EncryptedDocument>)> = loop {
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
                    let local_blob = self.docs.maybe_get(id, Some(hmac)).await?;
                    if local_blob.is_none() && !retried {
                        drop(tx);
                        // catch up our local db and try to read again
                        self.begin_tx().await.end();
                        retried = true;
                        continue;
                    }
                    break Some((hmac, key, local_blob));
                }
                None => break None,
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
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>, origin: Option<Uuid>,
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
        let pending = self
            .docs
            .insert_pending(target_id, hmac, &encrypted)
            .await?;

        // commit the result
        {
            let mut tx = self.begin_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata)
                .to_staged(&mut db.local_metadata)
                .to_lazy();
            pending.promote().await?;
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

        self.events.doc_written(target_id, Actor::User(origin));
        self.add_doc_event(activity::DocEvent::Write(target_id, get_time().0))
            .await?;

        Ok(hmac)
    }

    /// Writes `ours`, an edit of `base`, to a document that may have changed since `base` was
    /// read from it. Changes made since merge in as sync would merge them; where the document's
    /// type has no merge it's left untouched and this returns `ReReadRequired`. `ours` equal to
    /// `base` is no edit. Returns the document's hmac after and whether there were changes to merge.
    #[instrument(level = "debug", skip(self, base, ours), err(Debug))]
    pub async fn write_from_base(
        &self, id: Uuid, base: &[u8], ours: Vec<u8>, origin: Option<Uuid>,
    ) -> LbResult<(Option<DocumentHmac>, bool)> {
        let name = self.get_file_by_id(id).await?.name;
        loop {
            let (hmac, theirs) = self.read_document_with_hmac(id, false).await?;
            if theirs == ours || ours == base {
                return Ok((hmac, false));
            }
            let moved = theirs != base;
            let content = if moved {
                DocumentType::from_file_name_using_extension(&name)
                    .merge(base, &ours, &theirs)
                    .ok_or(LbErrKind::ReReadRequired)?
            } else {
                ours.clone()
            };
            match self.safe_write(id, hmac, content, origin).await {
                Ok(hmac) => return Ok((Some(hmac), moved)),
                // another process wrote between our read and write; merge with that too
                Err(err) if err.kind == LbErrKind::ReReadRequired => continue,
                Err(err) => return Err(err),
            }
        }
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

/// What a document's metadata records for `content` under its key.
pub(crate) fn document_hmac(key: &AESKey, content: &[u8]) -> LbResult<DocumentHmac> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|err| LbErrKind::Unexpected(format!("hmac creation error: {err:?}")))?;
    mac.update(content);
    Ok(mac.finalize().into_bytes().into())
}

pub(crate) fn compress_encrypt_document(
    key: &AESKey, content: &[u8],
) -> LbResult<(DocumentHmac, EncryptedDocument)> {
    let hmac = document_hmac(key, content)?;
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
