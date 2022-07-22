use crate::model::core_file::{Base, Local};
use crate::{CoreError, RepoSource, RequestContext};
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::tree_like::Stagable;
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn read_document(
        &mut self, source: RepoSource, id: Uuid,
    ) -> Result<DecryptedDocument, CoreError> {
        todo!()
    }
}
