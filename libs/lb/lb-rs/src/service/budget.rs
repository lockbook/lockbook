//! Time budgets that dev builds enforce by panicking, so an overrun names its culprit.

use std::time::Duration;

/// How many 120 Hz frames we tolerate before calling it slow.
pub const NOOB_FACTOR: u64 = 8;

/// The longest a frame may take on the UI thread.
pub const FRAME_BUDGET: Duration = Duration::from_micros((16_667 / 2) * NOOB_FACTOR);
