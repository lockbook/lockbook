#[derive(Serialize, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub args: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
pub enum CallbackMessage<S, T> {
    Status(S),
    Done(LbResult<T>),
}

impl RpcRequest {
    pub fn new(method: impl Into<String>, args: Option<Vec<u8>>) -> Self {
        Self {
            method: method.into(),
            args,
        }
    }
}

pub async fn call_rpc<T>(
    socket_address: SocketAddrV4,
    method: &str,
    args: Option<Vec<u8>>,
) -> LbResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let mut stream = TcpStream::connect(socket_address)
            .await
            .map_err(core_err_unexpected)?;
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

    let resp: T = bincode::deserialize(&resp_buf).map_err(core_err_unexpected)?;
    Ok(resp)
}

pub async fn call_rpc_with_callback<S, T, F>(
    socket_address: SocketAddrV4,
    method: &str,
    args: Option<Vec<u8>>,
    mut on_status: F,
) -> LbResult<T>
where
    S: DeserializeOwned,
    T: DeserializeOwned,
    F: FnMut(S),
{
    let mut stream = TcpStream::connect(socket_address)
            .await
            .map_err(core_err_unexpected)?;
    let req = RpcRequest::new(method, args);
    let req_bytes = bincode::serialize(&req).map_err(core_err_unexpected)?;
    let mut out = Vec::with_capacity(4 + req_bytes.len());
    out.extend_from_slice(&(req_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(&req_bytes);
    stream.write_all(&out).await.map_err(core_err_unexpected)?;

    loop {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.map_err(core_err_unexpected)?;
        let n = u32::from_be_bytes(len_buf) as usize;

        let mut payload = vec![0u8; n];
        stream.read_exact(&mut payload).await.map_err(core_err_unexpected)?;

        let msg: CallbackMessage<S, T> =
            bincode::deserialize(&payload).map_err(core_err_unexpected)?;
        match msg {
            CallbackMessage::Status(s) => on_status(s),
            CallbackMessage::Done(res) => return res,
        }
    }
}

pub async fn handle_connection(stream: TcpStream, lb: Arc<LbServer>) -> LbResult<()> {
    let mut stream = stream;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let msg_len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; msg_len];
    stream.read_exact(&mut buf).await?;

    let req: RpcRequest = bincode::deserialize(&buf).map_err(core_err_unexpected)?;
    let payload = dispatch(lb, req).await?;

    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    stream.write_all(&out).await?;

    Ok(())
}


pub async fn listen_for_connections(lb: Arc<LbServer>, listener: TcpListener) -> LbResult<()> { 
    loop {
        let (stream, _) = listener.accept().await
            .map_err(core_err_unexpected)?;

        let lb = lb.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, lb).await {
                eprintln!("Connection error: {e:?}");
            }
        });
    }
}

use crate::dispatch::dispatch;
use std::net::SocketAddrV4;
use std::sync::Arc;
use serde::de::DeserializeOwned;
use serde::{Serialize,Deserialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bincode;
use crate::{LbResult, LbServer};
use crate::model::errors::{core_err_unexpected};