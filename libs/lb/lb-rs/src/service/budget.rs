//! Time budgets that hosts enforce for dev accounts by panicking and exiting, so an overrun names its
//! culprit.

use std::env;
use std::sync::OnceLock;
use std::time::Duration;

/// Set to a whole number of 120 Hz frames to replace [DEFAULT_NOOB_FACTOR] for this process, or to
/// `0` to turn the budgets off. It does nothing where they aren't [ENFORCED].
pub static NOOB_FACTOR: &str = "LB_NOOB_FACTOR";

/// How many 120 Hz frames we tolerate before calling it slow.
pub const DEFAULT_NOOB_FACTOR: u64 = 48;

/// Whether hosts enforce the budgets in this build. Debug builds enforce neither.
pub const ENFORCED: bool = !cfg!(debug_assertions);

/// The longest a frame may take on the UI thread, if any.
pub fn frame_budget() -> Option<Duration> {
    static BUDGET: OnceLock<Option<Duration>> = OnceLock::new();
    *BUDGET.get_or_init(|| budget(noob_factor(env::var(NOOB_FACTOR).ok().as_deref())))
}

/// The longest a db lock may be held, if any. The lock is fair, so a long hold of either kind
/// queues every later reader behind the next writer, including the UI thread mid-frame.
pub fn lock_budget() -> Option<Duration> {
    frame_budget()
}

fn noob_factor(var: Option<&str>) -> u64 {
    var.and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_NOOB_FACTOR)
}

fn budget(noob_factor: u64) -> Option<Duration> {
    let frame = 16_667 / 2;
    (noob_factor > 0).then(|| Duration::from_micros(frame * noob_factor.min(u64::MAX / frame)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noob_factor_parses_or_defaults() {
        assert_eq!(noob_factor(None), DEFAULT_NOOB_FACTOR);
        assert_eq!(noob_factor(Some("")), DEFAULT_NOOB_FACTOR);
        assert_eq!(noob_factor(Some("-3")), DEFAULT_NOOB_FACTOR);
        assert_eq!(noob_factor(Some("lots")), DEFAULT_NOOB_FACTOR);
        assert_eq!(noob_factor(Some(" 100\n")), 100);
    }

    #[test]
    fn budget_follows_noob_factor() {
        assert_eq!(budget(DEFAULT_NOOB_FACTOR), Some(Duration::from_micros(399_984)));
        assert_eq!(budget(100), Some(Duration::from_micros(833_300)));
        assert_eq!(budget(0), None);
        assert!(budget(u64::MAX).is_some());
    }
}
