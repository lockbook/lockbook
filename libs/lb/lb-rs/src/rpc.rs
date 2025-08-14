use crate::dispatch::dispatch;
use crate::model::errors::core_err_unexpected;
use crate::{LbResult, LbServer};
use bincode;
use serde::{Deserialize, Serialize};
use std::net::SocketAddrV4;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Method {
    CreateAccount = 0,
    ImportAccount,
    ImportAccountPrivateKeyV1,
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
    Subscribe,
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

pub async fn call_rpc<T, R>(socket_address: SocketAddrV4, method: Method, args: T) -> LbResult<R>
where
    T: Serialize,
    R: for<'de> Deserialize<'de>,
{
    let mut stream = TcpStream::connect(socket_address)
        .await
        .map_err(core_err_unexpected)?;

    send_rpc_request(&mut stream, method, &args).await?;
    recv_rpc_response(&mut stream).await
}

pub async fn send_rpc_request<T: Serialize>(
    stream: &mut TcpStream, method: Method, args: &T,
) -> LbResult<()> {
    let method_id = method as u16;
    let body = bincode::serialize(args).map_err(core_err_unexpected)?;
    let len = body.len();

    let mut buf = Vec::with_capacity(2 + std::mem::size_of::<usize>() + len);
    buf.extend_from_slice(&method_id.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&body);

    stream.write_all(&buf).await.map_err(core_err_unexpected)?;
    Ok(())
}

pub async fn recv_rpc_response<R: for<'de> Deserialize<'de>>(
    stream: &mut TcpStream,
) -> LbResult<R> {
    let mut len_buf = [0u8; std::mem::size_of::<usize>()];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(core_err_unexpected)?;
    let resp_len = usize::from_be_bytes(len_buf);

    let mut resp_buf = vec![0u8; resp_len];
    stream
        .read_exact(&mut resp_buf)
        .await
        .map_err(core_err_unexpected)?;

    let result = bincode::deserialize::<R>(&resp_buf).map_err(core_err_unexpected)?;
    Ok(result)
}

impl LbServer {
    pub async fn listen_for_connections(&self, listener: TcpListener) -> LbResult<()> {
        let lb = Arc::new(self.clone());
        loop {
            let (stream, _) = listener.accept().await.map_err(core_err_unexpected)?;

            let lb = lb.clone();
            tokio::spawn(async move {
                if let Err(e) = lb.handle_connection(stream).await {
                    eprintln!("Connection error: {e:?}");
                }
            });
        }
    }
    async fn handle_connection(&self, stream: TcpStream) -> LbResult<()> {
        let lb = Arc::new(self.clone());
        let mut stream = stream;

        let mut id_buf = [0u8; 2];
        stream.read_exact(&mut id_buf).await?;
        let method_id = u16::from_le_bytes(id_buf);

        let mut len_buf = [0u8; std::mem::size_of::<usize>()];
        stream.read_exact(&mut len_buf).await?;
        let msg_len = usize::from_le_bytes(len_buf);

        let mut msg = vec![0u8; msg_len];
        stream.read_exact(&mut msg).await?;

        let method: Method = unsafe { std::mem::transmute(method_id) };
        match method {
            Method::Subscribe => {
                self.handle_subscription(stream).await?;
            }
            _ => {
                let response = dispatch(lb, method, &msg).await?;
                let mut out = Vec::with_capacity(std::mem::size_of::<usize>() + response.len());
                out.extend_from_slice(&(response.len()).to_be_bytes());
                out.extend_from_slice(&response);
                stream.write_all(&out).await?;
            }
        }
        Ok(())
    }
    async fn handle_subscription(&self, mut stream: TcpStream) -> LbResult<()> {
        let mut rx = self.subscribe();

        loop {
            match rx.recv().await {
                Ok(event) => {
                    let serialized = bincode::serialize(&event).map_err(core_err_unexpected)?;
                    let len = serialized.len();

                    stream.write_all(&len.to_be_bytes()).await?;
                    stream.write_all(&serialized).await?;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    eprintln!("Client lagged by {} events", count);
                }
            }
        }

        Ok(())
    }
}
