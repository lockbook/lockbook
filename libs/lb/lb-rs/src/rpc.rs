#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Method {
    CreateAccount = 0,
    ImportAccount,
    ImportAccountPrivateKeyV1,
    ImportAccountPrivateKeyV2,
    ImportAccountPhrase,
    ExportAccountPrivateKey,
    ExportAccountPrivateKeyV1,
    ExportAccountPrivateKeyV2,
    ExportAccountPhrase,
    ExportAccountQr,
    DeleteAccount,
    SuggestedDocs,
    DisappearAccount,
    DisappearFile,
    ListUsers,
    GetAccountInfo,
    ValidateAccount,
    ValidateServer,
    FileInfo,
    RebuildIndex,
    BuildIndex,
    SetUserTier,
    UpgradeAccountStripe,
    UpgradeAccountGooglePlay,
    UpgradeAccountAppStore,
    CancelSubscription,
    GetSubscriptionInfo,
    DebugInfo,
    ReadDocument,
    WriteDocument,
    ReadDocumentWithHmac,
    SafeWrite,
    CreateFile,
    RenameFile,
    MoveFile,
    Delete,
    Root,
    ListMetadatas,
    GetChildren,
    GetAndGetChildrenRecursively,
    GetFileById,
    LocalChanges,
    ImportFiles,
    ExportFile,
    ExportFileRecursively,
    TestRepoIntegrity,
    GetAccount,
    CreateLinkAtPath,
    CreateAtPath,
    GetByPath,
    GetPathById,
    ListPaths,
    ListPathsWithIds,
    ShareFile,
    GetPendingShares,
    RejectShare,
    CalculateWork,
    Sync,
    GetLastSyncedHuman,
    GetTimestampHumanString,
    GetUsage,
    GetUncompressedUsageBreakdown,
    GetUncompressedUsage,
    Search,
    Status,
    GetConfig,
    GetLastSynced,
    GetSearch,
    GetKeychain,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcRequest<T> {
    pub method: String,
    pub args: T,
}

#[derive(Serialize, Deserialize)]
pub enum CallbackMessage<S, T> {
    Status(S),
    Done(LbResult<T>),
}

impl<T> RpcRequest<T> {
    pub fn new(method: impl Into<String>, args: T) -> Self {
        Self {
            method: method.into(),
            args,
        }
    }
}

pub async fn call_rpc<T, R>(
    socket_address: SocketAddrV4,
    method: Method,
    args: T,
) -> LbResult<R>
where
    T: Serialize,
    R: for<'de> Deserialize<'de>,
{
    let mut stream = TcpStream::connect(socket_address)
        .await
        .map_err(core_err_unexpected)?;

    let msg = bincode::serialize(&args).map_err(core_err_unexpected)?;

    let method_id = method as u16;
    let msg_len = msg.len() as u32;

    let mut full_msg = Vec::with_capacity(2 + 4 + msg.len());
    full_msg.extend_from_slice(&method_id.to_le_bytes());
    full_msg.extend_from_slice(&msg_len.to_le_bytes());
    full_msg.extend_from_slice(&msg);

    stream.write_all(&full_msg).await.map_err(core_err_unexpected)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(core_err_unexpected)?;
    let resp_len = u32::from_be_bytes(len_buf);

    let mut resp_buf = vec![0u8; resp_len as usize];
    stream.read_exact(&mut resp_buf).await.map_err(core_err_unexpected)?;

    let resp: R = bincode::deserialize(&resp_buf).map_err(core_err_unexpected)?;
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

    let mut id_buf = [0u8; 2];
    stream.read_exact(&mut id_buf).await?;
    let method_id = u16::from_le_bytes(id_buf);

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let msg_len = u32::from_le_bytes(len_buf);

    let mut msg = vec![0u8; msg_len as usize];
    stream.read_exact(&mut msg).await?;

    let method: Method = unsafe { std::mem::transmute(method_id) };

    let response = dispatch(lb, method, &msg).await?;

    let mut out = Vec::with_capacity(4 + response.len());
    out.extend_from_slice(&(response.len() as u32).to_be_bytes());
    out.extend_from_slice(&response);
    stream.write_all(&out).await?;
    Ok(())
}

impl LbServer {
        pub async fn listen_for_connections(&self, listener: TcpListener) -> LbResult<()> { 
            let lb = Arc::new(self.clone());
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