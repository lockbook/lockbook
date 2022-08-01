use crate::repo::document_repo;
use crate::{CoreError, RepoSource, RequestContext};
use crate::{CoreResult, OneKey};
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::validate;
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn read_document(&mut self, id: Uuid) -> CoreResult<DecryptedDocument> {
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
        if meta.document_hmac().is_none() {
            return Ok(vec![]);
        }

        let maybe_encrypted_document =
            match document_repo::maybe_get(self.config, RepoSource::Local, meta.id())? {
                Some(local) => Some(local),
                None => document_repo::maybe_get(self.config, RepoSource::Base, meta.id())?,
            };

        let doc = match maybe_encrypted_document {
            Some(doc) => tree.decrypt_document(&id, &doc, account)?,
            None => return Err(CoreError::FileNonexistent),
        };

        Ok(doc)
    }

    pub fn write_document(&mut self, id: Uuid, content: &[u8]) -> Result<(), CoreError> {
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

        let (_, doc) = tree.update_document(&id, content, account)?;
        document_repo::insert(self.config, RepoSource::Local, id, &doc)?;
        Ok(())
    }
}
