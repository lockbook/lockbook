//! Workspace-wide monotonic sequence counter.
//!
//! Sources (inputs that downstream caches depend on) bump this counter on
//! write and store the resulting value in a sibling `_seq` field. Caches
//! stamp the seq values they observed at compute time; on lookup, they
//! compare stamps against current values and recompute on mismatch. One
//! counter per [`egui::Context`], cloned to constructors that bump.
//!
//! No callbacks, no observers — pull-based, checked at read.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use egui::{Context, Id};

/// Returns the workspace counter, creating it on first call. Cache the
/// returned `Arc` at construction; bumps are `arc.fetch_add(1, Relaxed)`.
pub fn ws_seq(ctx: &Context) -> Arc<AtomicU64> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("ws_seq"), || Arc::new(AtomicU64::new(1)))
            .clone()
    })
}
