impl Lb {
    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument>{
        match self {
            Lb::Direct(inner) => {
                inner.read_document(id,user_activity).await
            }
            Lb::Network(proxy) => {
                proxy.read_document(id,user_activity).await
            }
        }
    }
    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.write_document(id,content).await
            }
            Lb::Network(proxy) => {
                proxy.write_document(id,content).await
            }
        }
    }
    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)>{
        match self {
            Lb::Direct(inner) => {
                inner.read_document_with_hmac(id,user_activity).await
            }
            Lb::Network(proxy) => {
                proxy.read_document_with_hmac(id,user_activity).await
            }
        }
    }
    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac>{
        match self {
            Lb::Direct(inner) => {
                inner.safe_write(id,old_hmac,content).await
            }
            Lb::Network(proxy) => {
                proxy.safe_write(id,old_hmac,content).await
            }
        }
    }
}

use uuid::Uuid;

use crate::{model::{crypto::DecryptedDocument, file_metadata::DocumentHmac}, Lb, LbResult};