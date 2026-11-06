use crate::io::server::Server;
use std::collections::HashSet;

use crate::Lb;
use crate::model::ValidationFailure;
use crate::model::access_info::UserAccessMode;
use crate::model::clock::get_time;
use crate::model::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::model::doc_info::LocalVersion;
use crate::model::errors::{LbErr, LbErrKind, LbResult};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{DocumentHmac, FileType, Owner};
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
        self.write_local_version(id, None, content, Actor::User(None))
            .await?;
        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        let mut caught_up = false;
        let mut pulled = false;
        let mut pull_error: Option<LbErr> = None;

        let result = loop {
            let (key, info, known_to_server) = {
                let tx = self.ro_tx().await;
                let db = tx.db();
                let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

                let file = tree.find(&id)?;
                validate::is_document(file)?;
                if tree.calculate_deleted(&id)? {
                    return Err(LbErrKind::FileNonexistent.into());
                }

                (
                    tree.decrypt_key(&id, &self.keychain)?,
                    db.doc_info.get(&id).copied(),
                    db.base_metadata.get(&id).is_some(),
                )
            };
            let state = info.unwrap_or_default();
            let readable = state.local.map(|local| local.hmac).or(state.base_hmac());
            let blob = match readable {
                Some(hmac) => self.docs.maybe_get(id, Some(hmac)).await?,
                None => None,
            };

            let needs_pull = state.local.is_none()
                && (state.is_behind()
                    || (readable.is_some() && blob.is_none())
                    || (info.is_none() && known_to_server));
            if needs_pull && !pulled {
                pulled = true;
                match self.pull_doc(id).await {
                    Ok(()) => continue,
                    Err(err) => pull_error = Some(err),
                }
            }

            match (readable, blob) {
                (Some(hmac), Some(blob)) => {
                    break (Some(hmac), decrypt_payload(&key, &blob)?);
                }
                (None, _) => match pull_error {
                    Some(err) if state.remote.is_some() || (info.is_none() && known_to_server) => {
                        return Err(err);
                    }
                    _ => break (None, vec![]),
                },
                (Some(_), None) if !caught_up => {
                    caught_up = true;
                    self.begin_tx().await.end();
                    continue;
                }
                (Some(_), None) => {
                    return Err(pull_error.unwrap_or_else(|| LbErrKind::FileNonexistent.into()));
                }
            }
        };

        if user_activity {
            self.add_doc_event(activity::DocEvent::Read(id, get_time().0))
                .await?;
        }

        Ok(result)
    }

    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>, origin: Option<Uuid>,
    ) -> LbResult<DocumentHmac> {
        self.write_local_version(id, Some(old_hmac), &content, Actor::User(origin))
            .await
    }

    pub async fn document_hmac(&self, id: Uuid) -> LbResult<Option<DocumentHmac>> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        let tree = db.base_metadata.stage(&db.local_metadata);
        tree.find(&id)?;
        Ok(db.doc_info.get(&id).and_then(|info| info.current_hmac()))
    }

    async fn write_local_version(
        &self, id: Uuid, expected: Option<Option<DocumentHmac>>, content: &[u8], actor: Actor,
    ) -> LbResult<DocumentHmac> {
        let (target, key) = self.writable_document(id).await?;
        if let Some(expected) = expected {
            let tx = self.ro_tx().await;
            let current = tx
                .db()
                .doc_info
                .get(&target)
                .and_then(|info| info.current_hmac());
            if current != expected {
                return Err(LbErrKind::ReReadRequired.into());
            }
        }

        let (hmac, encrypted) = compress_encrypt_document(&key, content)?;
        let compressed_size = encrypted.value.len() as u64;
        let mut pending = Some(self.docs.stage(target, hmac, &encrypted).await?);

        {
            let mut tx = self.begin_tx().await;
            let db = tx.db();
            let mut info = db.doc_info.get(&target).copied().unwrap_or_default();
            if !expected.is_some_and(|expected| info.current_hmac() != expected) {
                if info.base.is_none() {
                    info.base = info.remote;
                }
                info.local = if Some(hmac) == info.base_hmac() {
                    None
                } else {
                    Some(LocalVersion { hmac, compressed_size, timestamp: get_time().0 as u64 })
                };
                if let Some(pending) = pending.take() {
                    self.docs.commit(pending).await?;
                }
                db.doc_info.insert(target, info)?;
                tx.end();
            }
        }
        if let Some(pending) = pending {
            self.docs.discard(pending).await;
            return Err(LbErrKind::ReReadRequired.into());
        }

        self.events.doc_written(target, actor);
        self.add_doc_event(activity::DocEvent::Write(target, get_time().0))
            .await?;

        Ok(hmac)
    }

    async fn writable_document(&self, id: Uuid) -> LbResult<(Uuid, AESKey)> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let target = match tree.find(&id)?.file_type() {
            FileType::Document | FileType::Folder => id,
            FileType::Link { target } => target,
        };
        validate::is_document(tree.find(&target)?)?;
        if tree.calculate_deleted(&target)? {
            return Err(LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(target)).into());
        }
        if tree.access_mode(Owner(self.keychain.get_pk()?), &target)? < Some(UserAccessMode::Write)
        {
            return Err(LbErrKind::InsufficientPermission.into());
        }
        Ok((target, tree.decrypt_key(&target, &self.keychain)?))
    }

    pub(crate) async fn cleanup(&self) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let stale: Vec<Uuid> = db
            .doc_info
            .iter()
            .filter(|(id, _)| {
                db.base_metadata.get(*id).is_none() && db.local_metadata.get(*id).is_none()
            })
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            db.doc_info.remove(&id)?;
        }

        let retained = db
            .doc_info
            .iter()
            .flat_map(|(id, info)| info.hmacs().map(move |hmac| (*id, hmac)))
            .collect::<HashSet<_>>();
        self.docs.retain(retained).await?;

        tx.end();
        Ok(())
    }
}

pub(crate) fn document_hmac(key: &AESKey, content: &[u8]) -> LbResult<DocumentHmac> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|err| LbErrKind::Unexpected(format!("hmac creation error: {err:?}")))?;
    mac.update(content);
    Ok(mac.finalize().into_bytes().into())
}

pub(crate) fn encrypt_payload(key: &AESKey, plain: &[u8]) -> LbResult<EncryptedDocument> {
    let compressed = compression_service::compress(plain)?;
    symkey::encrypt(key, &compressed)
}

pub(crate) fn encrypt_diff_payload(
    key: &AESKey, diff: &[u8], content: &[u8],
) -> LbResult<EncryptedDocument> {
    let level = compression_service::level_for_content(content);
    let compressed = compression_service::compress_with(diff, level)?;
    symkey::encrypt(key, &compressed)
}

pub(crate) fn decrypt_payload(key: &AESKey, encrypted: &EncryptedDocument) -> LbResult<Vec<u8>> {
    let compressed = symkey::decrypt(key, encrypted)?;
    compression_service::decompress(&compressed)
}

pub fn compress_encrypt_document(
    key: &AESKey, content: &[u8],
) -> LbResult<(DocumentHmac, EncryptedDocument)> {
    Ok((document_hmac(key, content)?, encrypt_payload(key, content)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::doc_diff::DocDiff;

    #[test]
    fn text_diffs_are_compressed() {
        let key = symkey::generate_key();
        let base = "an existing line of the note\n".repeat(100);
        let content = format!("{base}{}", "a pasted line of text\n".repeat(1000));
        let diff = DocDiff::compute(base.as_bytes(), content.as_bytes())
            .encode()
            .unwrap();
        let payload = encrypt_diff_payload(&key, &diff, content.as_bytes()).unwrap();
        assert!(payload.value.len() < diff.len() / 2);
        assert_eq!(decrypt_payload(&key, &payload).unwrap(), diff);
    }
}
