impl LbClient {
    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&(path.to_string(), target_id))
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "create_link_at_path", Some(args)).await
    }
    pub async fn create_at_path(&self, path: &str) -> LbResult<File>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&path.to_string())
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "create_at_path", Some(args)).await
    }
    pub async fn get_by_path(&self, path: &str) -> LbResult<File>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&path.to_string())
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "get_by_path", Some(args)).await
    }
    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "get_path_by_id", Some(args)).await
    }
    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&filter)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "list_paths", Some(args)).await
    }
    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>>{
       let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&filter)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "list_paths_with_ids", Some(args)).await
    }
}

use crate::lb_client::LbClient;
use crate::model::errors::core_err_unexpected;
use crate::rpc::call_rpc;
use tokio::net::TcpStream;
use uuid::Uuid;
use crate::{model::{file::File, path_ops::Filter}, LbResult};