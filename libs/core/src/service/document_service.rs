use crate::LbResult;
use crate::{CoreError, CoreState, Requester};
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::document_repo::{self};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::tree_like::TreeLike;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn read_document(&mut self, id: Uuid) -> LbResult<DecryptedDocument> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(&self.config, &id, account)?;

        if !self.is_insertion_capped(id) {
            self.db
                .doc_events
                .push(id, document_repo::DocEvents::Read(Utc::now().timestamp()))?;
        }

        Ok(doc)
    }

    pub(crate) fn write_document(&mut self, id: Uuid, content: &[u8]) -> LbResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let id = match tree.find(&id)?.file_type() {
            FileType::Document | FileType::Folder => id,
            FileType::Link { target } => target,
        };
        let encrypted_document = tree.update_document(&id, content, account)?;
        let hmac = tree.find(&id)?.document_hmac();
        document_repo::insert(&self.config, &id, hmac, &encrypted_document)?;

        if !self.is_insertion_capped(id) {
            self.db
                .doc_events
                .push(id, document_repo::DocEvents::Write(Utc::now().timestamp()))?;
        }
        Ok(())
    }

    pub(crate) fn cleanup(&mut self) -> LbResult<()> {
        self.db
            .base_metadata
            .stage(&mut self.db.local_metadata)
            .to_lazy()
            .delete_unreferenced_file_versions(&self.config)?;
        Ok(())
    }
}
