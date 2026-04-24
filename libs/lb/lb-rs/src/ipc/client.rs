//! Guest-side IPC client.
//!
//! `RemoteLb` is defined on every platform so `LbInner::Remote` can be an
//! unconditional enum variant — that's what lets the forwarders on
//! [`crate::Lb`] dispatch with cfg-free match arms. The real persistent-UDS
//! implementation is `#[cfg(unix)]`; on other platforms the `call` method
//! returns an "ipc not supported" error and the Guest variant is never
//! constructed in the first place (see `Lb::init`).

use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::ipc::protocol::Request;
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};

#[cfg(unix)]
use {
    crate::ipc::frame::{read_frame, write_frame},
    crate::ipc::protocol::Frame,
    std::collections::HashMap,
    std::io,
    std::path::Path,
    std::sync::atomic::{AtomicU64, Ordering},
    tokio::io::AsyncWriteExt,
    tokio::net::UnixStream,
    tokio::net::unix::OwnedWriteHalf,
    tokio::sync::{Mutex, oneshot},
    tokio::task::JoinHandle,
};

#[cfg(unix)]
type InFlight = Arc<Mutex<HashMap<u64, oneshot::Sender<Vec<u8>>>>>;

/// Guest-side handle. Cloning is cheap — all live state is behind `Arc`.
#[derive(Clone)]
pub struct RemoteLb {
    inner: Arc<Inner>,
}

struct Inner {
    /// Held for callers that still need access to the original `Config`
    /// (e.g., `writeable_path` for log paths). The host owns the actual db.
    config: Config,
    #[cfg(unix)]
    unix: UnixInner,
}

#[cfg(unix)]
struct UnixInner {
    writer: Mutex<OwnedWriteHalf>,
    seq: AtomicU64,
    in_flight: InFlight,
    /// Background reader task. Aborted on drop so the connection cleans up
    /// even if the host disappears.
    reader_task: JoinHandle<()>,
}

#[cfg(unix)]
impl Drop for UnixInner {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

impl RemoteLb {
    /// Connect to a host. Only meaningful on Unix; other platforms don't
    /// reach this path because `Lb::init`'s guest fallback is `cfg(unix)`.
    #[cfg(unix)]
    pub async fn connect(socket: &Path, config: Config) -> io::Result<Self> {
        let stream = crate::ipc::transport::connect(socket).await?;
        Self::from_stream(stream, config)
    }

    #[cfg(unix)]
    fn from_stream(stream: UnixStream, config: Config) -> io::Result<Self> {
        let (read_half, write_half) = stream.into_split();
        let in_flight: InFlight = Arc::new(Mutex::new(HashMap::new()));
        let reader_task = tokio::spawn(reader_loop(read_half, Arc::clone(&in_flight)));

        Ok(Self {
            inner: Arc::new(Inner {
                config,
                unix: UnixInner {
                    writer: Mutex::new(write_half),
                    seq: AtomicU64::new(0),
                    in_flight,
                    reader_task,
                },
            }),
        })
    }

    /// Configuration the guest was constructed with.
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    /// Invoke a method on the host.
    ///
    /// `req` is a typed [`Request`] variant — its discriminant tells the
    /// host which method to dispatch and carries that method's arguments.
    /// The host writes back a bincode-encoded `LbResult<Out>`; if the
    /// caller's `Out` disagrees with what the host wrote, bincode fails
    /// and the error surfaces as `LbErrKind::Unexpected`.
    #[cfg(unix)]
    pub async fn call<Out>(&self, req: Request) -> LbResult<Out>
    where
        Out: DeserializeOwned,
    {
        let u = &self.inner.unix;
        let seq = u.seq.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        u.in_flight.lock().await.insert(seq, tx);

        let frame = Frame::Request { seq, body: req };
        let bytes = bincode::serialize(&frame)
            .map_err(|e| LbErrKind::Unexpected(format!("ipc: serialize request: {e}")))?;

        {
            let mut writer = u.writer.lock().await;
            write_frame(&mut *writer, &bytes)
                .await
                .map_err(|e| LbErrKind::Unexpected(format!("ipc: write request: {e}")))?;
            writer
                .flush()
                .await
                .map_err(|e| LbErrKind::Unexpected(format!("ipc: flush request: {e}")))?;
        }

        let output_bytes = rx.await.map_err(|_| {
            LbErrKind::Unexpected("ipc: host disconnected before response".into())
        })?;

        let result: LbResult<Out> = bincode::deserialize(&output_bytes)
            .map_err(|e| LbErrKind::Unexpected(format!("ipc: deserialize response: {e}")))?;
        result
    }

    /// Non-Unix stub: `Lb::init` never constructs a Remote on these
    /// platforms, so this is unreachable — kept only so `LbInner::Remote`
    /// can be an unconditional variant and the forwarders stay cfg-free.
    #[cfg(not(unix))]
    pub async fn call<Out>(&self, _req: Request) -> LbResult<Out>
    where
        Out: DeserializeOwned,
    {
        Err(LbErrKind::Unexpected("ipc not supported on this platform".into()).into())
    }
}

#[cfg(unix)]
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
            Frame::Response { seq, output } => {
                if let Some(tx) = in_flight.lock().await.remove(&seq) {
                    let _ = tx.send(output);
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
