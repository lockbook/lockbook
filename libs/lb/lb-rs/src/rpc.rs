#[derive(Serialize, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub args: Option<Vec<u8>>,
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
    stream: &mut TcpStream,
    method: &str,
    args: Option<Vec<u8>>,
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

    let resp: T = bincode::deserialize(&resp_buf).map_err(core_err_unexpected)?;
    Ok(resp)
}

pub async fn dispatch(lb: Arc<LbServer>, req: RpcRequest) -> LbResult<Vec<u8>> {

    let raw = req.args.unwrap_or_default();
    let payload = match req.method.as_str() {
        "create_account" => {
            let (username, api_url, welcome): (String, String, bool) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.create_account(&username, &api_url, welcome).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account" => {
            let (key, maybe_url): (String, Option<String>) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.import_account(&key, maybe_url.as_deref()).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_private_key_v1" => {
            let account: crate::model::account::Account =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.import_account_private_key_v1(account).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_private_key_v2" => {
            let (pk_bytes, api_url): ( [u8; 32], String ) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let sk = SecretKey::parse(&pk_bytes)
                .map_err(core_err_unexpected)?;
            let res = lb.import_account_private_key_v2(sk, &api_url).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_phrase" => {
            let (phrase_vec, api_url): (Vec<String>, String) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let slice: Vec<&str> = phrase_vec.iter().map(|s| s.as_str()).collect();
            let phrase_arr: [&str; 24] = slice
                .try_into()
                .map_err(core_err_unexpected)?;
            
            let res = lb.import_account_phrase(phrase_arr, &api_url).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key" => {
            let res: String = lb.export_account_private_key()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key_v1" => {
            let res: String = lb.export_account_private_key_v1()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key_v2" => {
            let res: String = lb.export_account_private_key_v2()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_phrase" => {
            let res: String = lb.export_account_phrase()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_qr" => {
            let res: Vec<u8> = lb.export_account_qr()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "delete_account" => {
            lb.delete_account().await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        other => {
            return Err(LbErrKind::Unexpected(format!("Unknown method: {}", other)).into())
        }
    };

    Ok(payload)
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



use std::sync::Arc;
use libsecp256k1::SecretKey;
use serde::{Serialize,Deserialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use bincode;
use crate::{LbResult, LbServer};
use crate::model::errors::{core_err_unexpected};
use crate::model::errors::LbErrKind;