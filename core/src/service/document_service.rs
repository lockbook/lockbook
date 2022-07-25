use crate::repo::document_repo;
use crate::service::compression_service;
use crate::OneKey;
use crate::{CoreError, RepoSource, RequestContext};
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::validate;
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn read_document(&mut self, id: Uuid) -> Result<DecryptedDocument, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        if tree.calculate_deleted(&id)? {
            return Err(CoreError::FileNonexistent);
        }

        let meta = tree.find(&id)?;
        validate::is_document(&meta)?;

        let maybe_encrypted_document =
            match document_repo::maybe_get(self.config, RepoSource::Local, meta.id())? {
                Some(local) => Some(local),
                None => document_repo::maybe_get(self.config, RepoSource::Base, meta.id())?,
            };

        let doc = match maybe_encrypted_document {
            Some(doc) => {
                let compressed = tree.decrypt_document(&id, &doc, account)?;
                compression_service::decompress(&compressed)?
            }
            None => String::from("").into_bytes(),
        };

        Ok(doc)
    }

    pub fn write_document(&mut self, id: Uuid) -> Result<(), CoreError> {
        todo!()
    }
}
