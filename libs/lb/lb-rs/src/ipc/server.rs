//! Host-side IPC server.
//!
//! Stage 2 ships an accept loop that drains frames and currently rejects
//! every request — there are no `Request` variants to dispatch yet. Stage 3
//! replaces the `unimplemented!()` arm with the real `match req { … }` over
//! [`crate::LocalLb`] and adds event-stream multiplexing.

use std::io;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};

use crate::LocalLb;
use crate::ipc::frame::{read_frame, write_frame};
use crate::ipc::protocol::{Frame, Response};

/// Run the accept loop until the listener errors fatally. Spawns a task per
/// accepted connection.
///
/// `lb` is shared (`Arc`) across all connections — they all dispatch into
/// the same in-process state, exactly as if every guest were a thread in
/// the host.
pub async fn serve(listener: UnixListener, lb: Arc<LocalLb>) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let lb = Arc::clone(&lb);
                tokio::spawn(async move {
                    if let Err(err) = handle_conn(stream, lb).await {
                        // EOF / guest disconnect is normal; log lower level.
                        if err.kind() == io::ErrorKind::UnexpectedEof {
                            tracing::debug!("ipc guest disconnected");
                        } else {
                            tracing::warn!(?err, "ipc connection ended");
                        }
                    }
                });
            }
            Err(err) => {
                tracing::error!(?err, "ipc accept failed; aborting serve loop");
                return;
            }
        }
    }
}

async fn handle_conn(mut stream: UnixStream, _lb: Arc<LocalLb>) -> io::Result<()> {
    let (mut r, mut w) = stream.split();
    loop {
        let frame_bytes = read_frame(&mut r).await?;
        let frame: Frame = bincode::deserialize(&frame_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        match frame {
            Frame::Request { seq, body: _ } => {
                // Stage 3 dispatches `body` against `_lb` and constructs the
                // matching `Response::*` variant. Until then, the placeholder
                // arm exists only to keep the protocol enums non-empty;
                // surface that explicitly so a real guest fails loudly.
                let response = Frame::Response { seq, body: Response::__StagePlaceholder };
                let bytes = bincode::serialize(&response)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                write_frame(&mut w, &bytes).await?;
            }
            Frame::Response { .. } => {
                // Guests never send Response frames; treat as a protocol
                // violation.
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "guest sent a host-only frame",
                ));
            }
        }
    }
}
