use crate::shared::clock::get_time;
use crate::shared::crypto::DecryptedDocument;
use crate::shared::document_repo::DocumentService;
use crate::shared::file_like::FileLike;
use crate::shared::file_metadata::{DocumentHmac, FileType};
use crate::shared::tree_like::TreeLike;
use crate::LbResult;
use crate::{CoreError, CoreState, Requester};
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

    pub(crate) fn read_document_with_hmac(
        &mut self, id: Uuid,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(&self.docs, &id, account)?;
        let hmac = tree.find(&id)?.document_hmac().copied();

        self.add_doc_event(activity_service::DocEvent::Read(id, get_time().0))?;

        Ok((hmac, doc))
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
        self.cleanup()?;
        Ok(())
    }

    pub(crate) fn safe_write(
        &mut self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();

        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;
        let file = tree.find(&id)?;
        if file.document_hmac() != old_hmac.as_ref() {
            return Err(CoreError::ReReadRequired.into());
        }
        let id = match file.file_type() {
            FileType::Document | FileType::Folder => id,
            FileType::Link { target } => target,
        };
        // todo can we not borrow here?
        let encrypted_document = tree.update_document(&id, &content, account)?;
        let hmac = tree.find(&id)?.document_hmac();
        let hmac = *hmac.ok_or_else(|| {
            CoreError::Unexpected(format!("hmac missing for a document we just wrote {}", id))
        })?;
        self.docs.insert(&id, Some(&hmac), &encrypted_document)?;
        self.add_doc_event(activity_service::DocEvent::Write(id, get_time().0))?;

        self.cleanup()?;

        Ok(hmac)
    }

    pub(crate) fn cleanup(&mut self) -> LbResult<()> {
        if self.syncing {
            debug!("skipping doc cleanup due to active sync");
            return Ok(());
        }

        self.db
            .base_metadata
            .stage(&mut self.db.local_metadata)
            .to_lazy()
            .delete_unreferenced_file_versions(&self.docs)?;
        Ok(())
    }
}
