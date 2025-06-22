impl LbClient {
    pub async fn test_repo_integrity(&self) -> LbResult<Vec<Warning>>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "test_repo_integrity", None).await
    }
}

use crate::lb_client::LbClient;
use crate::model::errors::core_err_unexpected;
use crate::{model::errors::Warning, Lb, LbResult};
use crate::rpc::call_rpc;
use tokio::net::TcpStream;