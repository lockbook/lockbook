use crate::{CoreError, RequestContext, Requester};
use crate::{CoreResult, OneKey};
use chrono::Utc;
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::document_repo::{self, DocEvents};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::tree_like::TreeLike;
use uuid::Uuid;

const RATE_LIMIT_MILLIS: i64 = 60 * 1000;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn read_document(&mut self, id: Uuid) -> CoreResult<DecryptedDocument> {
        let mut tree = (&self.tx.base_metadata)
            .to_staged(&self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(self.config, &id, account)?;

        let mut doc_events = self.tx.docs_events.get(&id).unwrap_or(&Vec::new()).clone();
        doc_events.sort();
        let latest_event = doc_events
            .iter()
            .filter(|e| matches!(e, DocEvents::Read(_)))
            .last();

        let is_capped = match latest_event {
            Some(event) => Utc::now().timestamp() - event.timestamp() > RATE_LIMIT_MILLIS,
            None => true,
        };

        if !is_capped {
            doc_events.push(document_repo::DocEvents::Read(Utc::now().timestamp()));
            self.tx.docs_events.insert(id, doc_events);
        }

        Ok(doc)
    }

    pub fn write_document(&mut self, id: Uuid, content: &[u8]) -> CoreResult<()> {
        let mut tree = (&self.tx.base_metadata)
            .to_staged(&mut self.tx.local_metadata)
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

        let mut doc_events = self.tx.docs_events.get(&id).unwrap_or(&Vec::new()).clone();
        doc_events.sort();
        let latest_event = doc_events
            .iter()
            .filter(|e| matches!(e, DocEvents::Write(_)))
            .last();

        let is_capped = match latest_event {
            Some(event) => Utc::now().timestamp() - event.timestamp() > RATE_LIMIT_MILLIS,
            None => true,
        };

        if !is_capped {
            doc_events.push(document_repo::DocEvents::Read(Utc::now().timestamp()));
            self.tx.docs_events.insert(id, doc_events);
        }

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
