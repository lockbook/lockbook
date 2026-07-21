//! Elevated floating chrome — context menus, hover tips, and small cards.
//!
//! One visual family for anything that lifts off the canvas/panel:
//! **canvas fill · 1px line · soft shadow · tight radius**.
//!
//! Call sites should not invent their own `Frame`/`Shadow` for popovers; use
//! [`FloatingChrome`] (or the tip/menu helpers built on it) so chrome stays
//! consistent across the shell and workspace.

mod menu;
mod tip;

pub use menu::{MenuEntries, TextMenu, is_menu_open, show_menu, show_text_menu};
pub use tip::{TipPlacement, tip_lines, tip_text, tip_ui, tip_ui_rich};

use egui::{Color32, CornerRadius, Frame, Margin, Shadow, Stroke};

use crate::theme::palette_v2::ThemeExt;

// ── Shared metrics (points) ─────────────────────────────────────────────────

/// Outer corner radius — menus, tips, and small hover cards.
pub const RADIUS: u8 = 6;
/// Default inner padding for tips / compact cards.
pub const PAD: i8 = 10;
/// Menu-style tighter pad (matches shell context menu).
pub const MENU_PAD_X: i8 = 5;
pub const MENU_PAD_Y: i8 = 5;

/// Soft drop shadow shared by floating surfaces.
pub fn shadow() -> Shadow {
    Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(40),
    }
}

/// Resolved colors + builders for floating frames.
///
/// Prefer [`FloatingChrome::from_ctx`] so fill/stroke track the live theme.
#[derive(Clone, Copy, Debug)]
pub struct FloatingChrome {
    pub fill: Color32,
    pub stroke: Color32,
}

impl FloatingChrome {
    /// Canvas fill + hairline stroke from the active lockbook theme.
    pub fn from_ctx(ctx: &egui::Context) -> Self {
        let theme = ctx.get_lb_theme();
        Self { fill: theme.neutral_bg(), stroke: theme.neutral() }
    }

    /// Explicit colors (e.g. from shell `Tokens`).
    pub fn new(fill: Color32, stroke: Color32) -> Self {
        Self { fill, stroke }
    }

    /// Default tip/card frame (`PAD` margin).
    pub fn frame(self) -> Frame {
        self.frame_margin(Margin::same(PAD))
    }

    /// Menu frame (tighter pad).
    pub fn menu_frame(self) -> Frame {
        self.frame_margin(Margin::symmetric(MENU_PAD_X, MENU_PAD_Y))
    }

    /// Frame with custom margin (usage card, DnD float, etc.).
    pub fn frame_margin(self, margin: Margin) -> Frame {
        Frame::new()
            .inner_margin(margin)
            .corner_radius(CornerRadius::same(RADIUS))
            .fill(self.fill)
            .stroke(Stroke::new(1.0, self.stroke))
            .shadow(shadow())
    }

    /// Restyle stock `Popup` / menu chrome to match floating surfaces.
    /// Call at the start of a `Popup::show` body.
    pub fn apply_popup_style(self, ui: &mut egui::Ui) {
        let s = ui.style_mut();
        s.visuals.window_fill = self.fill;
        s.visuals.panel_fill = self.fill;
        s.visuals.window_stroke = Stroke::new(1.0, self.stroke);
        s.visuals.popup_shadow = shadow();
        s.visuals.window_shadow = shadow();
        s.visuals.menu_corner_radius = CornerRadius::same(RADIUS);
        s.visuals.window_corner_radius = CornerRadius::same(RADIUS);
        s.spacing.menu_margin = Margin::same(PAD);
        s.spacing.window_margin = Margin::same(PAD);
    }
}
