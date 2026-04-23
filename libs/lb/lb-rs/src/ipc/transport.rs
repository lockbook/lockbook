//! UDS transport — bind/listen on the host, dial on the guest.
//!
//! Gated to `cfg(unix)` because tokio does not currently expose async UDS
//! on Windows. See [`crate::ipc`] module docs for the Windows story.

use std::io;
use std::path::Path;

use tokio::net::{UnixListener, UnixStream};

/// Bind the IPC socket. Removes a stale socket file at `path` if one is
/// left over from a previous run.
///
/// Safe to clobber because the host calls this only after winning the db-rs
/// filesystem lock — anyone else still holding the lock has already exited
/// and can no longer be using the old socket. Stage 3 will probe-then-bind
/// to defend against bugs in the lock acquisition path.
pub async fn listen(path: &Path) -> io::Result<UnixListener> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    UnixListener::bind(path)
}

/// Connect to the host's IPC socket.
pub async fn connect(path: &Path) -> io::Result<UnixStream> {
    UnixStream::connect(path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn listen_then_connect() {
        let dir = TempDir::new().unwrap();
        let sock = dir.path().join("lb.sock");

        let listener = listen(&sock).await.unwrap();
        let accept = tokio::spawn(async move { listener.accept().await.map(|(s, _)| s) });

        let _client = connect(&sock).await.unwrap();
        let _server = accept.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn listen_clobbers_stale_socket() {
        let dir = TempDir::new().unwrap();
        let sock = dir.path().join("lb.sock");

        // Stale file from a "previous run".
        std::fs::write(&sock, b"").unwrap();
        assert!(sock.exists());

        let _listener = listen(&sock).await.unwrap();
        let _client = connect(&sock).await.unwrap();
    }
}
