use std::sync::{Arc, Mutex, RwLock};

use egui::{Response, Sense, Ui};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

use crate::{GlyphonRendererCallback, TextBufferArea};

/// An egui widget that renders text through glyphon so that emoji and
/// non-Latin scripts work correctly in file names.
pub struct GlyphonLabel<'a> {
    text: &'a str,
    color: egui::Color32,
    font_size: f32,
    /// Total row height in logical pixels, passed to glyphon metrics and used
    /// for the allocated height. Defaults to `font_size * 1.4`.
    line_height: Option<f32>,
    /// Wrapping / truncation limit in logical pixels. `f32::MAX` means a
    /// single unbounded line.
    max_width: f32,
    sense: Sense,
}

impl<'a> GlyphonLabel<'a> {
    pub fn new(text: &'a str, color: egui::Color32) -> Self {
        Self {
            text,
            color,
            font_size: 14.0,
            line_height: None,
            max_width: f32::MAX,
            sense: Sense::hover(),
        }
    }

    pub fn font_size(self, font_size: f32) -> Self {
        Self { font_size, ..self }
    }

    /// Set the row height used for glyph metrics and allocation.
    ///
    /// Pass the same value to any sibling `GlyphonTextEdit` so that the text
    /// baseline doesn't shift when toggling between display and rename.
    pub fn line_height(self, line_height: f32) -> Self {
        Self { line_height: Some(line_height), ..self }
    }

    pub fn max_width(self, max_width: f32) -> Self {
        Self { max_width, ..self }
    }

    pub fn sense(self, sense: Sense) -> Self {
        Self { sense, ..self }
    }

    /// Returns the rendered size in logical pixels without placing the widget.
    /// Useful when the width is needed before layout, such as sizing a rename
    /// field.
    pub fn measure(&self, ui: &egui::Ui) -> egui::Vec2 {
        self.shape(ui.ctx()).1
    }

    fn shape(&self, ctx: &egui::Context) -> (Arc<RwLock<Buffer>>, egui::Vec2) {
        let font_system = ctx
            .data(|d| d.get_temp::<Arc<Mutex<FontSystem>>>(egui::Id::NULL))
            .expect("GlyphonLabel used outside of a wgpu context");
        let ppi = ctx.pixels_per_point();
        let line_height = self.line_height.unwrap_or(self.font_size * 1.4);

        let mut fs = font_system.lock().unwrap();
        let mut buf = Buffer::new(&mut fs, Metrics::new(self.font_size * ppi, line_height * ppi));
        buf.set_size(&mut fs, Some(self.max_width * ppi), None);
        buf.set_text(
            &mut fs,
            self.text,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
        );
        buf.shape_until_scroll(&mut fs, false);

        let line_height_px = buf.metrics().line_height;
        let (width, lines) = buf
            .layout_runs()
            .fold((0.0f32, 0u32), |(w, n), r| (w.max(r.line_w), n + 1));
        let size = egui::vec2(width / ppi, lines as f32 * line_height_px / ppi);
        (Arc::new(RwLock::new(buf)), size)
    }
}

impl egui::Widget for GlyphonLabel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (buffer, text_size) = self.shape(ui.ctx());

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(text_size.x, self.line_height.unwrap_or(self.font_size * 1.4)),
            self.sense,
        );

        if ui.is_rect_visible(rect) {
            let c = self.color;
            let area = TextBufferArea::new(
                buffer,
                rect,
                glyphon::Color::rgba(c.r(), c.g(), c.b(), c.a()),
                ui.ctx(),
                rect,
            );
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    rect,
                    GlyphonRendererCallback::new(vec![area]),
                ));
        }

        response
    }
}
