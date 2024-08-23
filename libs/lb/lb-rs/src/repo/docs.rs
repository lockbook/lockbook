use crate::logic::{crypto::EncryptedDocument, file_metadata::DocumentHmac, SharedResult};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct AsyncDocs {}

impl AsyncDocs {
    pub async fn insert(
        &self, id: &Uuid, hmac: Option<&DocumentHmac>, document: &EncryptedDocument,
    ) -> SharedResult<()> {
        todo!()
    }

    pub async fn get(
        &self, id: Uuid, hmac: Option<DocumentHmac>,
    ) -> SharedResult<EncryptedDocument> {
        todo!()
    }

    pub async fn delete(
        &self, id: Uuid, hmac: Option<DocumentHmac>,
    ) -> SharedResult<EncryptedDocument> {
        todo!()
    }
}
