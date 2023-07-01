use crate::LbResult;
use crate::{CoreError, CoreState, Requester};
use lockbook_shared::clock::get_time;
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::document_repo::DocumentService;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::tree_like::TreeLike;
use uuid::Uuid;

use super::activity_service;

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    pub(crate) fn read_document(&mut self, id: Uuid) -> LbResult<DecryptedDocument> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(&self.docs, &id, account)?;

        self.add_doc_event(activity_service::DocEvent::Read(id, get_time().0))?;

        Ok(doc)
    }

    pub(crate) fn write_document(&mut self, id: Uuid, content: &[u8]) -> LbResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;

        let id = match tree.find(&id)?.file_type() {
            FileType::Document | FileType::Folder => id,
            FileType::Link { target } => target,
        };
        let encrypted_document = tree.update_document(&id, content, account)?;
        let hmac = tree.find(&id)?.document_hmac();
        self.docs.insert(&id, hmac, &encrypted_document)?;

        self.add_doc_event(activity_service::DocEvent::Write(id, get_time().0))?;

        Ok(())
    }

    pub(crate) fn cleanup(&mut self) -> LbResult<()> {
        self.db
            .base_metadata
            .stage(&mut self.db.local_metadata)
            .to_lazy()
            .delete_unreferenced_file_versions(&self.docs)?;
        Ok(())
    }
}
