//! members of this module are *consumers* of the subscription stream of lb-rs
#[cfg(not(target_family = "wasm"))]
pub mod search;
pub mod status;
