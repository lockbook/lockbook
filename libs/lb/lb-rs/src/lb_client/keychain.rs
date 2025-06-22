impl LbClient {
    pub async fn get_account(&self) -> LbResult<Account>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "get_account", None).await
    }
}

use crate::lb_client::LbClient;
use crate::model::account::Account;
use crate::model::errors::core_err_unexpected;
use crate::LbResult;
use crate::rpc::call_rpc;
use tokio::net::TcpStream;