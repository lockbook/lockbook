//! members of this module are *consumers* of the subscription stream of lb-rs
pub(crate) mod doc_sync;
#[cfg(all(unix, not(target_os = "ios")))]
pub(crate) mod ipc;
pub mod status;
pub mod syncer;
