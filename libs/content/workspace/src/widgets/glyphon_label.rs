use std::sync::{Arc, Mutex, RwLock};

use egui::{Response, Sense, Ui};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

use crate::{GlyphonRendererCallback, TextBufferArea};

/// A widget that renders a pre-shaped glyphon [`Buffer`].
///
/// **Two placement modes:**
///
/// - `ui.put(rect, GlyphonLabel::new(...))` — fills the given rect exactly.
///   The default mode; used for tab labels where the rect is pre-computed.
///
/// - `ui.add(GlyphonLabel::new(...).line_height(lh))` — measures the buffer's
///   natural text width and allocates `(text_width × lh)` inline. Use this
///   when the label lives inside a flowing layout such as `ui.horizontal`.
///
/// Adding `.sense(Sense::click())` makes the response report clicks, which is
/// how you build a glyphon-rendered link.
pub struct GlyphonLabel {
    buffer: Arc<RwLock<Buffer>>,
    color: egui::Color32,
    /// Interaction sense. Defaults to `Sense::hover()`.
    sense: Sense,
    /// When `Some(lh)`, the widget measures the buffer's natural text width and
    /// allocates `(text_width, lh)` rather than filling `ui.max_rect()`.
    line_height: Option<f32>,
}

impl GlyphonLabel {
    pub fn new(buffer: Arc<RwLock<Buffer>>, color: egui::Color32) -> Self {
        Self { buffer, color, sense: Sense::hover(), line_height: None }
    }

    /// Set the interaction sense.
    ///
    /// Use `Sense::click()` to get a clickable link-like label.
    pub fn sense(self, sense: Sense) -> Self {
        Self { sense, ..self }
    }

    /// Enable inline (natural-width) allocation.
    ///
    /// When set, the widget measures the buffer's rendered text width and
    /// allocates exactly `(text_width, line_height)` rather than filling
    /// `ui.max_rect()`.  Pass the same `line_height` value that was used in
    /// [`GlyphonLabel::shape_and_measure`].
    pub fn line_height(self, line_height: f32) -> Self {
        Self { line_height: Some(line_height), ..self }
    }
}

impl GlyphonLabel {
    /// Shape `text` into a single-line glyphon [`Buffer`] and measure its
    /// rendered width in logical pixels, all in one call.
    ///
    /// - `max_width` is in **logical pixels**; pass `f32::MAX` for unbounded
    ///   single-line measurement (e.g. when sizing a rename input field).
    /// - Returns `(buffer, width_logical_px)`. The buffer is ready to pass
    ///   directly to [`GlyphonLabel::new`]; discard it if you only need the
    ///   width.
    pub fn shape_and_measure(
        font_system: &Arc<Mutex<FontSystem>>, text: &str, font_size: f32, line_height: f32,
        max_width: f32, ppi: f32,
    ) -> (Arc<RwLock<Buffer>>, f32) {
        let mut fs = font_system.lock().unwrap();
        let mut buf = Buffer::new(&mut fs, Metrics::new(font_size * ppi, line_height * ppi));
        buf.set_size(&mut fs, Some(max_width * ppi), None);
        buf.set_text(&mut fs, text, &Attrs::new().family(Family::SansSerif), Shaping::Advanced);
        buf.shape_until_scroll(&mut fs, false);
        let width = buf.layout_runs().map(|r| r.line_w).fold(0.0f32, f32::max) / ppi;
        (Arc::new(RwLock::new(buf)), width)
    }
}

impl egui::Widget for GlyphonLabel {
    fn ui(self, ui: &mut Ui) -> Response {
        let ppi = ui.ctx().pixels_per_point();

        // Determine the allocation size.
        //
        // Natural mode: measure the buffer's rendered text width so the widget
        // only occupies the space the text actually needs when placed inline.
        //
        // Fill mode (default): occupy the full rect provided by the caller,
        // typically via `ui.put(rect, ...)`.
        let size = match self.line_height {
            Some(lh) => {
                let buf = self.buffer.read().unwrap();
                let w = buf.layout_runs().map(|r| r.line_w).fold(0.0f32, f32::max) / ppi;
                egui::vec2(w, lh)
            }
            None => ui.max_rect().size(),
        };

        let (rect, response) = ui.allocate_exact_size(size, self.sense);

        if ui.is_rect_visible(rect) {
            let c = self.color;
            let glyph_color = glyphon::Color::rgba(c.r(), c.g(), c.b(), c.a());

            // rect is both the draw origin and the clip boundary — text never
            // bleeds outside the allocated area.
            let area = TextBufferArea::new(
                self.buffer,
                rect,
                glyph_color,
                ui.ctx(),
                rect, // clip_rect
            );

            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    rect, // paint-callback bounds match widget rect exactly
                    GlyphonRendererCallback::new(vec![area]),
                ));
        }

        response
    }
}
