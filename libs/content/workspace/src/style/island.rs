//! Islands: floating rows of controls over a canvas (image and PDF viewers).
//! An off-canvas plate with the overlay shadow; the caller measures the row
//! and passes its outer rect.

use egui::{Align, Color32, Layout, Rect, Ui};

use crate::style::chrome::{Radius, control_height, overlay_shadow};
use crate::style::color::{Theme, ThemeExt};
use crate::style::layout::place_at;
use crate::style::space::Space;

/// Gap between the plate's edge and its controls.
pub fn pad() -> f32 {
    Space::Xxs.pts()
}

/// Outer height of a one-row island of [`control_height`] controls.
pub fn height() -> f32 {
    control_height() + 2.0 * pad()
}

/// Outer width of an island whose controls add up to `content_w`.
pub fn width(content_w: f32) -> f32 {
    content_w + 2.0 * pad()
}

/// The plate's fill; controls on it use this as their ground.
pub fn fill(t: &Theme) -> Color32 {
    t.neutral_bg_secondary()
}

/// Paints the plate on `rect` and lays out `add` left to right inside it.
pub fn island<R>(ui: &mut Ui, rect: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = ui.ctx().get_lb_theme();
    let radius = Radius::Control.corner();
    ui.painter().add(overlay_shadow(&t).as_shape(rect, radius));
    ui.painter().rect_filled(rect, radius, fill(&t));
    place_at(ui, rect.shrink(pad()), Layout::left_to_right(Align::Center), add).0
}
