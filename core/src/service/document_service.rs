use crate::{CoreError, RequestContext};
use crate::{CoreResult, OneKey};
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::lazy::LazyStaged1;
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn read_document(&mut self, id: Uuid) -> CoreResult<DecryptedDocument> {
        let tree = LazyStaged1::core_tree(
            self.find_owner(&id)?,
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(self.config, &id, account)?.1;

        Ok(doc)
    }

    pub fn write_document(&mut self, id: Uuid, content: &[u8]) -> CoreResult<()> {
        let tree = LazyStaged1::core_tree(
            self.find_owner(&id)?,
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        tree.write_document(self.config, &id, content, account)?;
        Ok(())
    }
}
