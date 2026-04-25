use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;

use serde::de::DeserializeOwned;
#[cfg(unix)]
use tokio::net::unix;
use tokio::sync::broadcast;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

use crate::ipc::protocol::Request;
use crate::model::account::Account;
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};
use crate::service::events::Event;

#[cfg(unix)]
use {
    crate::ipc::protocol::Frame, std::io, std::path::Path, std::sync::atomic::Ordering,
    tokio::net::UnixStream, tokio::net::unix::OwnedWriteHalf,
};

type InFlight = Arc<Mutex<HashMap<u64, oneshot::Sender<Vec<u8>>>>>;

const EVENT_CHANNEL_CAPACITY: usize = 10_000;

#[cfg_attr(not(unix), allow(dead_code))]
pub struct RemoteLb {
    config: Config,
    account: OnceLock<Account>,
    events: Arc<OnceLock<broadcast::Sender<Event>>>,
    #[cfg(unix)]
    writer: Mutex<OwnedWriteHalf>,
    seq: AtomicU64,
    in_flight: InFlight,
    reader_task: JoinHandle<()>,
}

impl Drop for RemoteLb {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

impl RemoteLb {
    #[cfg(unix)]
    pub async fn connect(socket: &Path, config: Config) -> io::Result<Arc<Self>> {
        let stream = UnixStream::connect(socket).await?;
        let (read_half, write_half) = stream.into_split();
        let in_flight: InFlight = Arc::new(Mutex::new(HashMap::new()));
        let events: Arc<OnceLock<broadcast::Sender<Event>>> = Arc::new(OnceLock::new());
        let reader_task =
            tokio::spawn(reader_loop(read_half, Arc::clone(&in_flight), Arc::clone(&events)));

        let me = Arc::new(Self {
            config,
            account: OnceLock::new(),
            events,
            writer: Mutex::new(write_half),
            seq: AtomicU64::new(0),
            in_flight,
            reader_task,
        });

        if let Ok(account) = me.call::<Account>(Request::GetAccount).await {
            me.cache_account(account);
        }

        Ok(me)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn get_account(&self) -> LbResult<&Account> {
        self.account
            .get()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    pub fn cache_account(&self, account: Account) {
        let _ = self.account.set(account);
    }

    pub fn subscribe(self: &Arc<Self>) -> broadcast::Receiver<Event> {
        let tx = self.events.get_or_init(|| {
            let (tx, _) = broadcast::channel::<Event>(EVENT_CHANNEL_CAPACITY);
            let me = Arc::clone(self);
            tokio::spawn(async move {
                if let Err(err) = me.call::<()>(Request::Subscribe).await {
                    tracing::warn!(?err, "ipc: Subscribe failed; events won't be relayed");
                }
            });
            tx
        });
        tx.subscribe()
    }

    pub async fn call<Out>(&self, req: Request) -> LbResult<Out>
    where
        Out: DeserializeOwned,
    {
        match self.try_call(req).await {
            Ok(v) => Ok(v),
            Err(RemoteCallError::HostUnavailable) => {
                Err(LbErrKind::Unexpected("ipc: host disconnected".into()).into())
            }
            Err(RemoteCallError::Other(e)) => Err(e),
        }
    }

    pub(crate) async fn try_call<Out>(&self, req: Request) -> Result<Out, RemoteCallError>
    where
        Out: DeserializeOwned,
    {
        #[cfg(not(unix))]
        {
            let _ = req;
            unreachable!("RemoteLb cannot be constructed on non-unix targets")
        }
        #[cfg(unix)]
        {
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = oneshot::channel();
            self.in_flight.lock().await.insert(seq, tx);

            let frame = Frame::Request { seq, body: req };
            {
                let mut writer = self.writer.lock().await;
                frame
                    .write(&mut *writer)
                    .await
                    .map_err(|_| RemoteCallError::HostUnavailable)?;
            }

            let output_bytes = rx.await.map_err(|_| RemoteCallError::HostUnavailable)?;

            let result: LbResult<Out> = bincode::deserialize(&output_bytes).map_err(|e| {
                RemoteCallError::Other(
                    LbErrKind::Unexpected(format!("ipc: deserialize response: {e}")).into(),
                )
            })?;
            result.map_err(RemoteCallError::Other)
        }
    }
}

pub(crate) enum RemoteCallError {
    HostUnavailable,
    Other(crate::model::errors::LbErr),
}

#[cfg(unix)]
async fn reader_loop(
    mut reader: unix::OwnedReadHalf, in_flight: InFlight,
    events: Arc<OnceLock<broadcast::Sender<Event>>>,
) {
    loop {
        let frame = match Frame::read(&mut reader).await {
            Ok(f) => f,
            Err(err) => {
                if err.kind() != io::ErrorKind::UnexpectedEof {
                    tracing::warn!(?err, "ipc reader: read failed");
                }
                break;
            }
        };
        match frame {
            Frame::Response { seq, output } => {
                if let Some(tx) = in_flight.lock().await.remove(&seq) {
                    let _ = tx.send(output);
                } else {
                    tracing::warn!(seq, "ipc reader: response for unknown seq");
                }
            }
            Frame::Event { stream_seq: _, body } => {
                if let Some(tx) = events.get() {
                    let _ = tx.send(body);
                }
            }
            Frame::EventEnd { stream_seq } => {
                tracing::debug!(stream_seq, "ipc: host closed event stream");
            }
            Frame::Request { .. } => {
                tracing::warn!("ipc reader: host sent a Request frame; protocol violation");
                break;
            }
        }
    }

    let mut map = in_flight.lock().await;
    map.clear();
}
