#[derive(Serialize, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub args: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct RpcResponse<T> {
    pub result: LbResult<T>,
}

impl RpcRequest {
    pub fn new(method: impl Into<String>, args: Vec<u8>) -> Self {
        Self {
            method: method.into(),
            args,
        }
    }
}

pub async fn call_rpc<T>(
    stream: &mut TcpStream,
    method: &str,
    args: Vec<u8>,
) -> LbResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let req = RpcRequest::new(method, args);
    let encoded = bincode::serialize(&req).map_err(core_err_unexpected)?;
    let len = encoded.len() as u32;

    let mut full_msg = Vec::with_capacity(4 + encoded.len());
    full_msg.extend_from_slice(&len.to_be_bytes());
    full_msg.extend_from_slice(&encoded);

    stream.write_all(&full_msg).await.map_err(core_err_unexpected)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(core_err_unexpected)?;
    let resp_len = u32::from_be_bytes(len_buf);

    let mut resp_buf = vec![0u8; resp_len as usize];
    stream.read_exact(&mut resp_buf).await.map_err(core_err_unexpected)?;

    let resp: RpcResponse<T> = bincode::deserialize(&resp_buf).map_err(core_err_unexpected)?;
    resp.result
}

use serde::{Serialize,Deserialize};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bincode;
use crate::{LbResult};
use crate::model::errors::{core_err_unexpected};