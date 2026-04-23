//! Guest-side IPC client.
//!
//! Stage 2: a thin connection holder + send/recv-frame helpers. Stage 3
//! adds:
//! - sequence-number allocation,
//! - a request → response map shared between a writer task and a reader task,
//! - an event-stream demultiplexer that fans `Frame::Event` frames out to
//!   per-stream `Receiver<Event>`s,
//! - one `RemoteLb::method(...)` per `Lb` API method that constructs the
//!   matching `Request::*` variant, awaits the response, and unwraps it.

use std::io;
use std::path::Path;

use tokio::net::UnixStream;

use crate::ipc::frame::{read_frame, write_frame};
use crate::ipc::protocol::Frame;

/// Stage 2 placeholder for the guest-side handle.
///
/// Stage 3 expands this into a full client with background reader/writer
/// tasks. For now it just owns the connection and exposes raw frame I/O so
/// integration tests can exercise the socket end-to-end.
pub struct RemoteLb {
    stream: UnixStream,
}

impl RemoteLb {
    pub async fn connect(socket: &Path) -> io::Result<Self> {
        let stream = crate::ipc::transport::connect(socket).await?;
        Ok(Self { stream })
    }

    /// Send one bincode-encoded `Frame`. Stage 3 hides this behind
    /// per-method async forwarders.
    pub async fn send_frame(&mut self, frame: &Frame) -> io::Result<()> {
        let bytes = bincode::serialize(frame)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let (_, mut w) = self.stream.split();
        write_frame(&mut w, &bytes).await
    }

    /// Receive one `Frame`. Stage 3 will move this into a background reader
    /// task and dispatch to per-seq oneshot channels.
    pub async fn recv_frame(&mut self) -> io::Result<Frame> {
        let (mut r, _) = self.stream.split();
        let bytes = read_frame(&mut r).await?;
        bincode::deserialize(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
