//! Guest-side IPC client.
//!
//! `RemoteLb` holds one persistent UDS connection to the host and demuxes
//! responses by their request `seq`. Construction spawns a background
//! reader task that drains `Frame::Response`s from the socket and dispatches
//! each into a per-request `oneshot` channel. Per-method forwarders live on
//! [`crate::Lb`] and call into [`RemoteLb::call`].

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

use crate::ipc::frame::{read_frame, write_frame};
use crate::ipc::protocol::{Frame, Request, Response};
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};

type InFlight = Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>>;

/// Guest-side handle. Cloning is cheap — all live state is behind `Arc`.
#[derive(Clone)]
pub struct RemoteLb {
    inner: Arc<Inner>,
}

struct Inner {
    /// Held for callers that still need access to the original `Config`
    /// (e.g., `writeable_path` for log paths). The host owns the actual db.
    config: Config,
    writer: Mutex<OwnedWriteHalf>,
    seq: AtomicU64,
    in_flight: InFlight,
    /// Background reader task. Aborted on drop so the connection cleans up
    /// even if the host disappears.
    reader_task: JoinHandle<()>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

impl RemoteLb {
    pub async fn connect(socket: &Path, config: Config) -> io::Result<Self> {
        let stream = crate::ipc::transport::connect(socket).await?;
        Self::from_stream(stream, config)
    }

    fn from_stream(stream: UnixStream, config: Config) -> io::Result<Self> {
        let (read_half, write_half) = stream.into_split();
        let in_flight: InFlight = Arc::new(Mutex::new(HashMap::new()));
        let reader_task = tokio::spawn(reader_loop(read_half, Arc::clone(&in_flight)));

        Ok(Self {
            inner: Arc::new(Inner {
                config,
                writer: Mutex::new(write_half),
                seq: AtomicU64::new(0),
                in_flight,
                reader_task,
            }),
        })
    }

    /// Configuration the guest was constructed with. Mirrors the host's
    /// `LocalLb::config` for the small number of callers (debug output,
    /// log paths) that read it without an RPC.
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    /// Send `body`, await the matching response. The seq is allocated
    /// inside this method so callers don't need to think about it.
    pub async fn call(&self, body: Request) -> LbResult<Response> {
        let seq = self.inner.seq.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.inner.in_flight.lock().await.insert(seq, tx);

        let frame = Frame::Request { seq, body };
        let bytes = bincode::serialize(&frame).map_err(|e| {
            LbErrKind::Unexpected(format!("serialize ipc request: {e}"))
        })?;

        {
            let mut writer = self.inner.writer.lock().await;
            write_frame(&mut *writer, &bytes).await.map_err(|e| {
                LbErrKind::Unexpected(format!("write ipc request: {e}"))
            })?;
            writer.flush().await.map_err(|e| {
                LbErrKind::Unexpected(format!("flush ipc request: {e}"))
            })?;
        }

        rx.await.map_err(|_| {
            // The reader task either errored or was dropped — either way the
            // host is gone or the connection is broken. Per the plan this is
            // a fail-fast error; the caller can re-`init` to retry.
            LbErrKind::Unexpected("ipc host disconnected before response".into()).into()
        })
    }
}

async fn reader_loop(mut reader: tokio::net::unix::OwnedReadHalf, in_flight: InFlight) {
    loop {
        let frame_bytes = match read_frame(&mut reader).await {
            Ok(b) => b,
            Err(err) => {
                if err.kind() != io::ErrorKind::UnexpectedEof {
                    tracing::warn!(?err, "ipc reader: socket read failed");
                }
                break;
            }
        };
        let frame: Frame = match bincode::deserialize(&frame_bytes) {
            Ok(f) => f,
            Err(err) => {
                tracing::warn!(?err, "ipc reader: malformed frame");
                break;
            }
        };
        match frame {
            Frame::Response { seq, body } => {
                if let Some(tx) = in_flight.lock().await.remove(&seq) {
                    let _ = tx.send(body);
                } else {
                    tracing::warn!(seq, "ipc reader: response for unknown seq");
                }
            }
            Frame::Request { .. } => {
                tracing::warn!("ipc reader: host sent a Request frame; protocol violation");
                break;
            }
        }
    }

    // Drain any remaining waiters so they fail fast instead of hanging.
    let mut map = in_flight.lock().await;
    map.clear();
}
