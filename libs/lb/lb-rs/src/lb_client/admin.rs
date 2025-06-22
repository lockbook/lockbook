impl LbClient {
    pub async fn disappear_account(&self, username: &str) -> LbResult<()>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&username.to_string())
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "disappear_account", Some(args)).await
    }
    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "disappear_file", Some(args)).await
    }
    pub async fn list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&filter.map(|s| s))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "list_users", Some(args)).await
    }
    pub async fn get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&identifier)
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "get_account_info", Some(args)).await
    }
    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&username.to_string())
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "validate_account", Some(args)).await
    }
    pub async fn validate_server(&self) -> LbResult<AdminValidateServer>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "validate_server",None).await
    }
    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "file_info", Some(args)).await
    }
    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&index)
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "rebuild_index", Some(args)).await
    }
    pub async fn set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(username.to_string(),info))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "set_user_tier", Some(args)).await
    }
}

use crate::lb_client::LbClient;
use crate::{model::errors::core_err_unexpected};
use tokio::net::TcpStream;
use crate::Uuid;
use crate::rpc::call_rpc;
use crate::{model::{account::Username, api::{AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse, AdminSetUserTierInfo, AdminValidateAccount, AdminValidateServer, ServerIndex}}, Lb, LbResult};