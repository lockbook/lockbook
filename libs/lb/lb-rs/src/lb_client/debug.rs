impl LbClient {
    pub async fn debug_info(&self, os_info: String) -> LbResult<String>{
       let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&os_info)
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "debug_info", Some(args)).await
    }
}

use crate::lb_client::LbClient;
use crate::{model::errors::core_err_unexpected, LbResult};
use tokio::net::TcpStream;
use crate::rpc::call_rpc;