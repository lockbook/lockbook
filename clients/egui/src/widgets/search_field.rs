//! Compact in-sheet text field (emoji-safe via Glyphon).
//!
//! Used by the move sheet filter and share username entry. Finder / settings-
//! filter style: small type, optional leading icon, tight frame. Distinct from
//! the workspace “Open Quickly” hero box (22pt, large margins).

use egui::{
    Align, CornerRadius, FontId, Frame, Id, Layout, Margin, Response, Sense, Stroke, Ui, vec2,
};
use workspace_rs::widgets::GlyphonTextEdit;

use crate::theme::icons;
use crate::theme::tokens::Tokens;

/// Text size — body-adjacent, not title (tree names are 14).
const FONT: f32 = 13.0;
const LINE_H: f32 = 18.0;
const ICON_SIZE: f32 = 14.0;
const PAD_X: i8 = 10;
/// Vertical pad so outer height is exactly [`HEIGHT`] (7 + 18 + 7).
const PAD_Y: i8 = 7;
const RADIUS: u8 = 8;
const ICON_GAP: f32 = 6.0;
const CLEAR_W: f32 = 18.0;

/// Outer control height (padding + line). Share sheet / move sheet align
/// sibling buttons and pickers to this.
pub const HEIGHT: f32 = (PAD_Y as f32) * 2.0 + LINE_H;

/// Compact search field with a leading magnifying glass.
pub fn show(
    ui: &mut Ui,
    t: &Tokens,
    id: impl Into<Id>,
    text: &mut String,
    hint: &str,
) -> Response {
    show_with_leading(ui, t, id, text, hint, Some(icons::SEARCH))
}

/// Same chrome as [`show`], with a custom leading Phosphor glyph (`None` = no icon).
pub fn show_with_leading(
    ui: &mut Ui,
    t: &Tokens,
    id: impl Into<Id>,
    text: &mut String,
    hint: &str,
    leading: Option<&'static str>,
) -> Response {
    let id = id.into();
    let text_id = id.with("edit");
    let focused = ui.memory(|m| m.has_focus(text_id));

    // Keep buffer in sync before layout (same pattern as find / rename).
    let _ = GlyphonTextEdit::process_events(ui, text_id, text);

    let (fill, stroke) = if focused {
        (t.canvas(), Stroke::new(1.0, t.text_muted()))
    } else {
        (t.surface_raised(), Stroke::new(1.0, t.line()))
    };

    // Pin outer height so neighbors (Add, mode picker) can match exactly.
    ui.set_min_height(HEIGHT);
    ui.set_max_height(HEIGHT);

    let out = Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::symmetric(PAD_X, PAD_Y))
        .show(ui, |ui| {
            ui.set_min_height(LINE_H);
            ui.set_max_height(LINE_H);
            // LTR: [icon?][edit………………][×] — avoids RTL TextEdit offset quirks.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = ICON_GAP;
                ui.set_min_height(LINE_H);
                ui.set_max_height(LINE_H);

                if let Some(icon) = leading {
                    let icon_g = ui.painter().layout_no_wrap(
                        icon.into(),
                        icons::font(ICON_SIZE),
                        t.text_muted(),
                    );
                    let (icon_rect, _) = ui.allocate_exact_size(
                        vec2(icon_g.size().x, LINE_H.max(ICON_SIZE)),
                        Sense::hover(),
                    );
                    ui.painter().galley(
                        icon_rect.left_top()
                            + egui::vec2(0.0, (icon_rect.height() - icon_g.size().y) * 0.5),
                        icon_g,
                        t.text_muted(),
                    );
                }

                let show_clear = !text.is_empty();
                let clear_reserve = if show_clear {
                    CLEAR_W + ICON_GAP
                } else {
                    0.0
                };
                let edit_w = (ui.available_width() - clear_reserve).max(40.0);

                // Glyphon fills a fixed width so cursor/scroll math is correct.
                let edit_resp = ui
                    .allocate_ui_with_layout(
                        vec2(edit_w, LINE_H),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.set_min_width(edit_w);
                            ui.set_max_width(edit_w);
                            GlyphonTextEdit::new(text)
                                .id(text_id)
                                .font_size(FONT)
                                .line_height(LINE_H)
                                .hint_text(hint)
                                .show(ui)
                        },
                    )
                    .inner;

                if show_clear {
                    let clear_id = id.with("clear");
                    let (cr, cresp) =
                        ui.allocate_exact_size(vec2(CLEAR_W, LINE_H), Sense::click());
                    let ch = ui.ctx().animate_bool(clear_id, cresp.hovered());
                    let clear_ink = t.text_muted().lerp_to_gamma(t.fg(), 0.4 * ch);
                    let xg = ui.painter().layout_no_wrap(
                        "×".into(),
                        FontId::proportional(15.0),
                        clear_ink,
                    );
                    ui.painter()
                        .galley(cr.center() - xg.size() / 2.0, xg, clear_ink);
                    if cresp.clicked() {
                        text.clear();
                        ui.memory_mut(|m| m.request_focus(text_id));
                    }
                }

                // Click empty frame chrome → focus the field.
                if edit_resp.clicked() {
                    ui.memory_mut(|m| m.request_focus(text_id));
                }
                edit_resp
            })
            .inner
        });

    out.inner
}
