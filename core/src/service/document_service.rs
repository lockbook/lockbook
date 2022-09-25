use crate::{CoreError, RequestContext, Requester};
use crate::{CoreResult, OneKey};
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::tree_like::Stagable;
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn read_document(&mut self, id: Uuid) -> CoreResult<DecryptedDocument> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(self.config, &id, account)?.1;

        Ok(doc)
    }

    pub fn write_document(&mut self, id: Uuid, content: &[u8]) -> CoreResult<()> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        tree.write_document(self.config, &id, content, account)?;
        Ok(())
    }
}
