impl LbClient {
    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(id,user_activity))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "read_document", Some(args)).await
    }
    pub async fn write_document(
        &self,
        id: Uuid,
        content: &[u8],
    ) -> LbResult<()> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(id, content))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "write_document", Some(args)).await
    }

    pub async fn read_document_with_hmac(
        &self,
        id: Uuid,
        user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(id, user_activity))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream,"read_document_with_hmac",Some(args)).await
    }

    pub async fn safe_write(
        &self,
        id: Uuid,
        old_hmac: Option<DocumentHmac>,
        content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(id, old_hmac, content))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "safe_write", Some(args)).await
    }
}

use crate::lb_client::LbClient;
use crate::model::crypto::DecryptedDocument;
use crate::model::file_metadata::DocumentHmac;
use crate::{model::errors::core_err_unexpected, LbResult};
use tokio::net::TcpStream;
use uuid::Uuid;
use crate::rpc::call_rpc;