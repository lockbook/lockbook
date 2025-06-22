impl LbClient {
    pub async fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&settings)
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "suggested_docs", Some(args)).await
    }
}

use crate::lb_client::LbClient;
use crate::service::activity::RankingWeights;
use crate::{model::errors::core_err_unexpected, LbResult};
use tokio::net::TcpStream;
use crate::Uuid;
use crate::rpc::call_rpc;