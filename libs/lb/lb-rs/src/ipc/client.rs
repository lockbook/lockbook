//! Guest-side IPC client.
//!
//! `RemoteLb` is defined on every platform so `LbInner::Remote` can be an
//! unconditional enum variant — that's what lets the forwarders on
//! [`crate::Lb`] dispatch with cfg-free match arms. The real persistent-UDS
//! implementation is `#[cfg(unix)]`; on other platforms the `call` method
//! returns an "ipc not supported" error and the Guest variant is never
//! constructed in the first place (see `Lb::init`).
//!
//! # Subscriber relay (lazy)
//!
//! A guest that never subscribes pays nothing for the subscription path:
//! no `Request::Subscribe` on the wire, no broadcast channel allocation,
//! no event traffic. The relay is set up on the *first* call to
//! [`RemoteLb::subscribe`]: that call wins a `OnceLock` init race, spawns
//! a task that sends `Request::Subscribe` to the host, and creates the
//! local broadcast. Subsequent `subscribe()` calls just hand out more
//! receivers from the same channel — still one host-side subscription.
//!
//! The reader task checks the `OnceLock` on every `Frame::Event`; if the
//! channel hasn't been initialized the event is dropped. This is a cheap
//! atomic load per event and keeps the reader path branch-free when no
//! subscribers exist.

use std::sync::Arc;
use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use tokio::sync::broadcast;

use crate::ipc::protocol::Request;
use crate::model::account::Account;
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};
use crate::service::events::Event;

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

/// Bound on the local event broadcast — matches the host-side
/// `EventSubs` capacity. Lagged receivers see `RecvError::Lagged` and can
/// resync.
const EVENT_CHANNEL_CAPACITY: usize = 10_000;

/// Guest-side state. Held inside `Lb::Remote` as `Arc<RemoteLb>` so
/// cloning the `Lb` enum is cheap. The struct itself isn't `Clone` — it
/// owns non-cloneable resources (the writer mutex, the in-flight oneshot
/// map, the reader task handle) directly.
pub struct RemoteLb {
    /// Held for callers that still need access to the original `Config`
    /// (e.g., `writeable_path` for log paths). The host owns the actual db.
    config: Config,
    /// Account cache. Seeded at connect time and refreshed when a
    /// successful `create_account` / `import_account*` call returns from
    /// the host. Lets `get_account()` return `&Account` synchronously
    /// without an IPC round-trip on the hot path.
    account: OnceLock<Account>,
    /// Lazy local broadcast. Initialized on first `subscribe()` call along
    /// with the host-side `Request::Subscribe`. Guests that never
    /// subscribe pay nothing — no channel buffer, no wire traffic.
    events: Arc<OnceLock<broadcast::Sender<Event>>>,
    #[cfg(unix)]
    writer: Mutex<OwnedWriteHalf>,
    #[cfg(unix)]
    seq: AtomicU64,
    #[cfg(unix)]
    in_flight: InFlight,
    /// Background reader task. Aborted on drop so the connection cleans up
    /// even if the host disappears.
    #[cfg(unix)]
    reader_task: JoinHandle<()>,
}

#[cfg(unix)]
impl Drop for RemoteLb {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

impl RemoteLb {
    /// Connect to a host. Only meaningful on Unix; other platforms don't
    /// reach this path because `Lb::init`'s guest fallback is `cfg(unix)`.
    #[cfg(unix)]
    pub async fn connect(socket: &Path, config: Config) -> io::Result<Arc<Self>> {
        let stream = crate::ipc::transport::connect(socket).await?;
        let me = Self::from_stream(stream, config)?;
        // Best-effort: seed the account cache so `get_account()` works
        // synchronously. A fresh install with no signed-in account returns
        // `AccountNonexistent` here — fine, the cache stays empty until
        // `create_account` / `import_account*` populates it.
        if let Ok(account) = me.call::<Account>(Request::GetAccount).await {
            me.cache_account(account);
        }
        Ok(me)
    }

    #[cfg(unix)]
    fn from_stream(stream: UnixStream, config: Config) -> io::Result<Arc<Self>> {
        let (read_half, write_half) = stream.into_split();
        let in_flight: InFlight = Arc::new(Mutex::new(HashMap::new()));
        let events: Arc<OnceLock<broadcast::Sender<Event>>> = Arc::new(OnceLock::new());
        let reader_task =
            tokio::spawn(reader_loop(read_half, Arc::clone(&in_flight), Arc::clone(&events)));

        Ok(Arc::new(Self {
            config,
            account: OnceLock::new(),
            events,
            writer: Mutex::new(write_half),
            seq: AtomicU64::new(0),
            in_flight,
            reader_task,
        }))
    }

    /// Configuration the guest was constructed with.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the cached `Account` if the guest has one (set at connect
    /// time or after a successful create/import call). Returns
    /// `LbErrKind::AccountNonexistent` otherwise — same surface as
    /// `LocalLb::get_account`.
    pub fn get_account(&self) -> LbResult<&Account> {
        self.account
            .get()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    /// Stash an `Account` in the local cache so subsequent `get_account()`
    /// calls return it. Idempotent — second `set` is a no-op since the
    /// account is invariant for a session.
    pub fn cache_account(&self, account: Account) {
        let _ = self.account.set(account);
    }

    /// Subscribe to the relayed event stream.
    ///
    /// The first caller wins the `OnceLock` init race: that call allocates
    /// the broadcast channel *and* spawns a task that sends
    /// `Request::Subscribe` to the host. Subsequent callers just get more
    /// receivers from the same channel — still one subscription on the
    /// host side. Guests that never call this pay nothing.
    ///
    /// Takes `&Arc<Self>` so the spawned host-Subscribe task can hold its
    /// own ref to the connection without needing the type to be `Clone`.
    pub fn subscribe(self: &Arc<Self>) -> broadcast::Receiver<Event> {
        let tx = self.events.get_or_init(|| {
            let (tx, _) = broadcast::channel::<Event>(EVENT_CHANNEL_CAPACITY);

            // Kick off the host-side subscription. Failure is logged, not
            // fatal — the guest still has a working (empty) receiver.
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
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.in_flight.lock().await.insert(seq, tx);

        let frame = Frame::Request { seq, body: req };
        let bytes = bincode::serialize(&frame)
            .map_err(|e| LbErrKind::Unexpected(format!("ipc: serialize request: {e}")))?;

        {
            let mut writer = self.writer.lock().await;
            write_frame(&mut *writer, &bytes)
                .await
                .map_err(|e| LbErrKind::Unexpected(format!("ipc: write request: {e}")))?;
            writer
                .flush()
                .await
                .map_err(|e| LbErrKind::Unexpected(format!("ipc: flush request: {e}")))?;
        }

        let output_bytes = rx
            .await
            .map_err(|_| LbErrKind::Unexpected("ipc: host disconnected before response".into()))?;

        let result: LbResult<Out> = bincode::deserialize(&output_bytes)
            .map_err(|e| LbErrKind::Unexpected(format!("ipc: deserialize response: {e}")))?;
        result
    }

    /// Non-Unix stub: `Lb::init` never constructs a Remote on these
    /// platforms, so this is unreachable — kept only so `Lb::Remote` can
    /// be an unconditional variant and the forwarders stay cfg-free.
    #[cfg(not(unix))]
    pub async fn call<Out>(&self, _req: Request) -> LbResult<Out>
    where
        Out: DeserializeOwned,
    {
        Err(LbErrKind::Unexpected("ipc not supported on this platform".into()).into())
    }
}

#[cfg(unix)]
async fn reader_loop(
    mut reader: tokio::net::unix::OwnedReadHalf, in_flight: InFlight,
    events: Arc<OnceLock<broadcast::Sender<Event>>>,
) {
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
            Frame::Event { stream_seq: _, body } => {
                // If no one has called `subscribe()` yet the channel isn't
                // initialized and the event is dropped — which is correct:
                // the host shouldn't be sending us Events before we sent
                // Subscribe, and we only send Subscribe during channel init.
                if let Some(tx) = events.get() {
                    let _ = tx.send(body);
                }
            }
            Frame::EventEnd { stream_seq } => {
                tracing::debug!(stream_seq, "ipc: host closed event stream");
                // We don't terminate on EventEnd — the request/response
                // channel is still useful even after the subscription
                // ends. Receivers will simply stop seeing events.
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
