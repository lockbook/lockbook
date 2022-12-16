use crate::{CoreError, RequestContext, Requester};
use crate::{CoreResult, OneKey};
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::document_repo;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::tree_like::TreeLike;
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn read_document(&mut self, id: Uuid) -> CoreResult<DecryptedDocument> {
        let mut tree = self
            .tx
            .base_metadata
            .stage(&self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(self.config, &id, account)?;

        Ok(doc)
    }

    pub fn write_document(&mut self, id: Uuid, content: &[u8]) -> CoreResult<()> {
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let id = match tree.find(&id)?.file_type() {
            FileType::Document | FileType::Folder => id,
            FileType::Link { target } => target,
        };
        let encrypted_document = tree.update_document(&id, content, account)?;
        let hmac = tree.find(&id)?.document_hmac();
        document_repo::insert(self.config, &id, hmac, &encrypted_document)?;

        Ok(())
    }

    pub fn cleanup(&mut self) -> CoreResult<()> {
        self.tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .delete_unreferenced_file_versions(self.config)?;
        Ok(())
    }
}
