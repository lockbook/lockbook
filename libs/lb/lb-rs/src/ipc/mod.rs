//! Inter-process communication for lb-rs host/guest fallback.
//!
//! Today every `lb-rs` client takes an exclusive db-rs filesystem lock on the
//! database folder. When a second client opens the same folder it fails. With
//! IPC, the second client becomes a **guest**: it connects to a Unix Domain
//! Socket inside the same folder and forwards every `Lb` API call to the
//! **host** process (the one that holds the lock).
//!
//! Access control falls out for free — the socket file inherits the parent
//! directory's permissions, so anyone who can read/write the db folder can
//! read/write the socket, and nobody else can.
//!
//! # Stages
//!
//! - **Stage 2 (this commit)**: transport (UDS), framing, empty
//!   request/response protocol enums. The module compiles, the wire format
//!   is set, but no `Lb` method yet has a `Request`/`Response` variant.
//!   `Lb::init` is unchanged and the IPC server is never started.
//! - **Stage 3**: populate `Request`/`Response` with one variant per `Lb`
//!   method, wire the server's dispatch to `LocalLb`, build `RemoteLb`
//!   that implements those forwarders, and add the host/guest race to
//!   `Lb::init`.
//! - **Follow-up (deferred)**: the subscriber API (`Lb::subscribe`) needs
//!   its own treatment — a long-lived event stream doesn't fit the
//!   request/response shape. Expected to land as additional `Frame`
//!   variants plus a separate event enum.
//!
//! # Platform support
//!
//! UDS is gated to `cfg(unix)` because tokio does not currently expose
//! `UnixListener` on Windows. Windows 10 1803+ has stdlib UDS via
//! `std::os::windows::net`; Stage 3 can wrap those with `spawn_blocking` or
//! a third-party crate (`interprocess`, `uds_windows`) if Windows host/guest
//! is required. Until then, on Windows the second process simply fails to
//! acquire the db-rs lock as it does today.

#[cfg(unix)]
pub mod client;
pub mod frame;
pub mod protocol;
#[cfg(unix)]
pub mod server;
#[cfg(unix)]
pub mod transport;

use std::path::{Path, PathBuf};

/// Filename of the UDS socket inside `Config::writeable_path`.
///
/// Lives next to the db-rs files so its filesystem permissions track the
/// parent directory's permissions.
pub const SOCKET_FILENAME: &str = "lb.sock";

/// Resolve the IPC socket path for a given lb-rs database folder.
pub fn socket_path(writeable_path: impl AsRef<Path>) -> PathBuf {
    writeable_path.as_ref().join(SOCKET_FILENAME)
}
