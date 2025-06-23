impl LbClient {
    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&(id,username.to_string(),mode))
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "share_file", Some(args)).await
    }
    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "get_pending_shares", None).await
    }
    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "reject_share", Some(args)).await
    }
}

use crate::lb_client::LbClient;
use crate::model::errors::core_err_unexpected;
use crate::rpc::call_rpc;
use tokio::net::TcpStream;
use uuid::Uuid;
use crate::{model::{errors::LbErr, file::{File, ShareMode}}, LbResult};