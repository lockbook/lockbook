//! members of this module are *consumers* of the subscription stream of lb-rs
#[cfg(not(any(target_family = "wasm", target_os = "ios")))]
mod ipc;
pub mod status;
pub mod syncer;
