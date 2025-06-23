impl LbClient {
    pub async fn get_usage(&self) -> LbResult<UsageMetrics>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "get_usage", None).await
    }
    pub async fn get_uncompressed_usage_breakdown(&self) -> LbResult<HashMap<Uuid, usize>>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "get_uncompressed_usage_breakdown", None).await
    }
    pub async fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "geget_uncompressed_usaget_account", None).await
    }
}

use crate::lb_client::LbClient;
use crate::model::errors::core_err_unexpected;
use crate::rpc::call_rpc;
use tokio::net::TcpStream;
use std::collections::HashMap;
use uuid::Uuid;
use crate::{service::usage::{UsageItemMetric, UsageMetrics}, LbResult};