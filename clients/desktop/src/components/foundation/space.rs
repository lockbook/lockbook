//! Closed space ramp. Talk in token names (`md` → `lg`); pts are the implementation.

use egui::Color32;
use workspace_rs::theme::palette_v2::Palette;

use super::color::Theme;

/// Ordered space steps. Values are points (logical px).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    /// 2 pt — hair gaps (kbd glyph pairs, tight stacks).
    Xxs,
    /// 5 pt
    Xs,
    /// 10 pt
    Sm,
    /// 15 pt
    Md,
    /// 25 pt
    Lg,
    /// 40 pt
    Xl,
}

impl Space {
    pub const fn pts(self) -> f32 {
        match self {
            Space::Xxs => 2.0,
            Space::Xs => 5.0,
            Space::Sm => 10.0,
            Space::Md => 15.0,
            Space::Lg => 25.0,
            Space::Xl => 40.0,
        }
    }

    /// Fixed bright hue per space step (mode-independent, for the F2 overlay).
    /// Opaque so the same RGB shows on light or dark canvas (alpha over black
    /// was reading as “darker in dark mode”).
    pub fn overlay_fill(self, t: &Theme) -> Color32 {
        let p = match self {
            Space::Xxs => Palette::Red,
            Space::Xs => Palette::Blue,
            Space::Sm => Palette::Green,
            Space::Md => Palette::Yellow,
            Space::Lg => Palette::Magenta,
            Space::Xl => Palette::Cyan,
        };
        t.bright.get_color(p)
    }
}

/// Button / field inset tokens.
pub mod control {
    use super::Space;

    pub const PAD_X: Space = Space::Sm;
    pub const PAD_Y: Space = Space::Xs;
    pub const ICON_GAP: Space = Space::Xs;
    pub const SHORTCUT_GAP: Space = Space::Sm;
    pub const PART_GAP: Space = Space::Xxs;
}
