//! Time budgets that hosts enforce for dev accounts by panicking and exiting, so an overrun names its
//! culprit.

use std::time::Duration;

/// How many 120 Hz frames we tolerate before calling it slow.
pub const NOOB_FACTOR: u64 = 8;

/// Whether hosts enforce the budgets in this build. Debug builds enforce neither.
pub const ENFORCED: bool = !cfg!(debug_assertions);

/// The longest a frame may take on the UI thread.
pub const FRAME_BUDGET: Duration = Duration::from_micros((16_667 / 2) * NOOB_FACTOR);

/// The longest a db lock may be held. The lock is fair, so a long hold of either kind queues every
/// later reader behind the next writer, including the UI thread mid-frame.
pub const LOCK_BUDGET: Duration = FRAME_BUDGET;
