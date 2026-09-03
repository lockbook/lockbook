//! Titleband geometry shared by the toolbar, tab strip, and sidebar min width.

use crate::components::{Space, control_height};

/// y-center of the title row.
pub const HEADER_CENTER: f32 = 20.0;
/// Full title / tab strip height.
pub const HEADER_H: f32 = HEADER_CENTER * 2.0;

/// Left edge of the floating toolbar.
pub const TOGGLE_X: f32 = 10.0;

const TOOLBAR_GAP: f32 = 4.0;
/// Win11 caption cell width (~46). Height follows [`HEADER_H`]. Grab after
/// tabs is one cell so the empty chrome is as easy to hit as a window button.
pub const CAPTION_CELL_W: f32 = 46.0;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub const CAPTION_W: f32 = CAPTION_CELL_W;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LINUX_HIT_W: f32 = 36.0;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LINUX_GAP: f32 = 2.0;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LINUX_WASH: f32 = 28.0;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LINUX_GLYPH: f32 = 12.0;
/// Optical drop so the minimize dash sits low in the box (Adwaita / COSMIC).
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LINUX_MIN_SHIFT: f32 = 3.0;

const TOOLBAR_ICONS: usize = 3;
const NAV_ICONS: usize = 2;
const TITLEBAR_GLYPH: f32 = 18.0;

pub fn titlebar_glyph() -> f32 {
    TITLEBAR_GLYPH
}

pub fn toolbar_gap() -> f32 {
    TOOLBAR_GAP
}

/// Hit / hover-wash size for titleband icons.
pub fn icon_hit_size() -> f32 {
    let air = Space::Xs.pts() * 2.0;
    (HEADER_H - air).min(control_height()).max(1.0)
}

pub fn icon_size() -> f32 {
    icon_hit_size()
}

pub fn toolbar_cluster_w() -> f32 {
    let n = TOOLBAR_ICONS as f32;
    n * icon_size() + (n - 1.0) * TOOLBAR_GAP
}

pub fn nav_cluster_w() -> f32 {
    let n = NAV_ICONS as f32;
    n * icon_size() + (n - 1.0) * TOOLBAR_GAP
}

pub fn group_gap() -> f32 {
    Space::Md.pts()
}

pub fn left_chrome_w() -> f32 {
    toolbar_cluster_w() + group_gap() + nav_cluster_w()
}

fn tab_gap_after_toolbar() -> f32 {
    Space::Md.pts()
}

/// Window-x of the first tab when the sidebar is closed. Sidebar min width
/// matches this so a min-width split is the same x — tabs do not move.
pub fn controls_right() -> f32 {
    TOGGLE_X + left_chrome_w() + tab_gap_after_toolbar()
}

pub fn tab_drag_gap() -> f32 {
    CAPTION_CELL_W
}

pub fn tab_left_inset(sidebar_w: f32) -> f32 {
    (controls_right() - sidebar_w.max(0.0)).max(0.0)
}

pub fn tab_right_inset() -> f32 {
    caption_cluster_w()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn caption_cluster_w() -> f32 {
    3.0 * CAPTION_W
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn caption_cluster_w() -> f32 {
    3.0 * LINUX_HIT_W + 2.0 * LINUX_GAP + Space::Sm.pts()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_forty() {
        assert!((HEADER_H - 40.0).abs() < 0.01);
        assert!((HEADER_CENTER * 2.0 - HEADER_H).abs() < 0.01);
    }
}
