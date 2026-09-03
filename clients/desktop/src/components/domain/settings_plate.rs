//! Settings modal plate geometry (center + top-safe inset).
//!
//! Pure layout — no `ShellApp`. Used by settings UI and layout tests.

use egui::{Pos2, Rect};

const SETTINGS_W: f32 = 760.0;
const SETTINGS_H: f32 = 560.0;
/// Outer margin from screen edges.
const SCREEN_MARGIN: f32 = 40.0;
/// Captions live in the titleband (right); no extra top inset.
const TOP_SAFE: f32 = 0.0;

/// Preferred plate size for a given screen (shared with tests).
pub fn plate_size_for_screen(screen: Rect) -> (f32, f32) {
    let max_w = (screen.width() - SCREEN_MARGIN).max(1.0);
    // Leave TOP_SAFE above + half margin below so a top-clamped plate still fits.
    let max_h = (screen.height() - SCREEN_MARGIN - TOP_SAFE).max(1.0);
    (SETTINGS_W.min(max_w).max(1.0), SETTINGS_H.min(max_h).max(1.0))
}

/// Top-left of the plate: horizontally centered; vertically centered **unless**
/// that would sit closer than [`TOP_SAFE`] to the top.
///
/// We do **not** use `Area` `Align2::CENTER_*` + offset hacks — egui centers in
/// the full screen rect and ignores safe insets (see design house rules).
pub fn plate_origin_for_screen(screen: Rect, plate_w: f32, plate_h: f32) -> Pos2 {
    let side = SCREEN_MARGIN * 0.5;
    let mut left = screen.center().x - plate_w * 0.5;
    // Keep side air when the plate is narrower than the screen.
    let min_left = screen.left() + side;
    let max_left = (screen.right() - side - plate_w).max(min_left);
    left = left.clamp(min_left, max_left);

    let ideal_top = screen.center().y - plate_h * 0.5;
    let min_top = screen.top() + TOP_SAFE;
    // Prefer center; never violate the top safe inset.
    let top = ideal_top.max(min_top);
    Pos2::new(left, top)
}
