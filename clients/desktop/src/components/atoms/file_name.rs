//! File **names** always go through glyphon so emoji / complex scripts shape.
//!
//! Do **not** paint file names with `painter.layout` / `ui.label` + proportional
//! fonts — Twemoji is intentionally not an egui proportional fallback (it fights
//! phosphor PUA and color-font handling). Use:
//! - display: [`paint`] / [`measure`] ([`GlyphonLabel`])
//! - edit: [`workspace_rs::widgets::GlyphonTextEdit`] (see [`super::field::Field`])

use egui::{Color32, Rect, Ui, pos2, vec2};
use workspace_rs::widgets::{GlyphonLabel, TextOverflow};

use crate::components::foundation::typography::TypeRole;

/// Body-size metrics matching [`TypeRole::Body`] / control line box.
pub fn body_font_size() -> f32 {
    TypeRole::Body.size()
}

pub fn body_line_height() -> f32 {
    TypeRole::Body.line_height()
}

/// Natural width of `name` at body metrics (unbounded).
pub fn measure(ui: &Ui, name: &str) -> f32 {
    measure_sized(ui, name, body_font_size(), body_line_height(), f32::MAX)
}

/// Width after shaping. When `max_w` is finite, end-ellipsis is applied so the
/// measured width matches what [`paint`] will draw.
pub fn measure_sized(ui: &Ui, name: &str, font_size: f32, line_height: f32, max_w: f32) -> f32 {
    let mut label = GlyphonLabel::new(name, Color32::WHITE)
        .font_size(font_size)
        .line_height(line_height)
        .max_width(max_w);
    if max_w.is_finite() && max_w < 1.0e20 {
        label = label.text_overflow(TextOverflow::EndEllipsis);
    }
    label.measure(ui).x
}

/// Paint a file name left-aligned in `slot` (full height = line box).
///
/// Uses end-ellipsis when the shaped width would exceed `slot.width()`.
/// Returns the drawn width (for trailing chrome placement).
pub fn paint(
    ui: &mut Ui, name: &str, color: Color32, slot: Rect, font_size: f32, line_height: f32,
) -> f32 {
    let max_w = slot.width().max(0.0);
    if max_w < 0.5 || name.is_empty() {
        return 0.0;
    }
    let shaped = GlyphonLabel::new(name, color)
        .font_size(font_size)
        .line_height(line_height)
        .max_width(max_w)
        .text_overflow(TextOverflow::EndEllipsis)
        .build(ui.ctx());
    let drawn_w = shaped.size.x.min(max_w);
    // Vertically center the line box in `slot` (slot may be taller for two-line rows).
    let y = slot.center().y - line_height / 2.0;
    let text_rect = Rect::from_min_size(pos2(slot.left(), y), vec2(drawn_w, line_height));
    let clip = text_rect.intersect(ui.clip_rect()).intersect(slot);
    if clip.width() > 0.5 && clip.height() > 0.5 && ui.is_rect_visible(clip) {
        let area = shaped.text_area(text_rect, ui.ctx(), clip);
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                clip,
                workspace_rs::GlyphonRendererCallback::new(vec![area]),
            ));
    }
    drawn_w
}

/// Convenience: body metrics, end-ellipsis.
pub fn paint_body(ui: &mut Ui, name: &str, color: Color32, slot: Rect) -> f32 {
    paint(ui, name, color, slot, body_font_size(), body_line_height())
}
