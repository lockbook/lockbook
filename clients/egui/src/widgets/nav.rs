//! Sidebar chrome primitives, sharing the file tree's visual language: a
//! frameless icon button for the toolbar, a full-width nav row (icon + label)
//! for quick actions, and a partial-width hairline that separates sections.

use egui::{FontId, Rangef, Response, Sense, Stroke, Ui, pos2, vec2};

use crate::theme::{icons, tokens::Tokens};

/// Side of the square `icon_button`; exported so callers can center it.
pub const ICON_BUTTON_SIZE: f32 = 26.0;

/// A frameless square icon button — toolbar affordance. Hover eases in a neutral
/// state layer and firms the glyph from muted to full ink.
pub fn icon_button(ui: &mut Ui, t: &Tokens, icon: &str) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ICON_BUTTON_SIZE, ICON_BUTTON_SIZE), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let fill = t.fg().gamma_multiply(0.06 * hover);
    if fill.a() > 0 {
        ui.painter().rect_filled(rect, 6.0, fill);
    }
    let color = t.text_muted().lerp_to_gamma(t.fg(), hover);
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(18.0), color);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, color);
    resp
}

/// A full-width row styled like a file-tree row — phosphor icon + label with the
/// same neutral state layer. For sidebar quick actions.
pub fn nav_row(ui: &mut Ui, t: &Tokens, icon: &str, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let fill = if resp.is_pointer_button_down_on() {
        t.fg().gamma_multiply(0.10)
    } else {
        t.fg().gamma_multiply(0.05 * hover)
    };
    if fill.a() > 0 {
        ui.painter().rect_filled(rect, 6.0, fill);
    }

    let ink = t.text_muted().lerp_to_gamma(t.fg(), hover);
    let cy = rect.center().y;
    let mut x = rect.left() + 12.0;
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(16.0), ink);
    ui.painter().galley(pos2(x, cy - g.size().y / 2.0), g, ink);
    x += 24.0;
    let g = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(14.0), ink);
    ui.painter().galley(pos2(x, cy - g.size().y / 2.0), g, ink);
    resp
}

/// A horizontal hairline inset from both edges — a light section divider that
/// doesn't reach the sidebar's sides.
pub fn hairline(ui: &mut Ui, t: &Tokens) {
    let inset = 14.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        Rangef::new(rect.left() + inset, rect.right() - inset),
        rect.center().y,
        Stroke::new(1.0, t.line()),
    );
}
