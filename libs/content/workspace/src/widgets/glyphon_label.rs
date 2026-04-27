use std::sync::{Arc, Mutex, RwLock};

use egui::{Response, Sense, Ui};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};

use crate::widgets::glyphon_cache::{
    GlyphonCache, GlyphonCacheKey, GlyphonCacheSpan, GlyphonFontFamily,
};
use crate::{GlyphonRendererCallback, TextBufferArea};

/// Per-span styling: bold flag and optional color override.
#[derive(Clone, Copy)]
pub struct SpanStyle {
    pub bold: bool,
    pub color: Option<egui::Color32>,
}

/// A text label rendered through glyphon so that emoji, non-Latin scripts, and
/// mixed bold/normal spans work correctly.
///
/// Optionally includes a right-aligned hint (e.g. a keyboard shortcut) that is
/// rendered in a separate color and smaller font. When the label is placed in a
/// rect wider than its natural size, extra space goes between the main text and
/// the hint.
///
/// Two rendering paths:
/// - **egui layout**: `ui.add(label)` — allocates space and paints immediately.
/// - **manual placement**: `label.build(ctx)` → [`ShapedLabel`] — returns the
///   shaped buffers and measured size so callers can position it at an arbitrary
///   rect and batch multiple labels into a single paint callback.
pub struct GlyphonLabel<'a> {
    spans: Vec<(&'a str, SpanStyle)>,
    color: egui::Color32,
    hint: Option<(&'a str, egui::Color32)>,
    font_size: f32,
    /// Total row height in logical pixels, passed to glyphon metrics and used
    /// for the allocated height. Defaults to `font_size * 1.4`.
    line_height: Option<f32>,
    /// Wrapping / truncation limit in logical pixels. `f32::MAX` means a
    /// single unbounded line.
    max_width: f32,
    sense: Sense,
}

/// The result of shaping a [`GlyphonLabel`]. Contains the sized glyphon buffers
/// ready for placement at an arbitrary screen rect.
pub struct ShapedLabel {
    main: ShapedBuffer,
    hint: Option<ShapedBuffer>,
    /// Minimum size in logical pixels: main width + gap + hint width.
    pub size: egui::Vec2,
}

struct ShapedBuffer {
    buffer: Arc<RwLock<Buffer>>,
    size: egui::Vec2,
    color: glyphon::Color,
}

/// Gap between main text and hint in logical pixels.
const HINT_GAP: f32 = 8.0;

impl ShapedLabel {
    /// Size of the hint text, if any.
    pub fn hint_size(&self) -> Option<egui::Vec2> {
        self.hint.as_ref().map(|h| h.size)
    }

    /// Creates [`TextBufferArea`]s positioned within `rect`.
    /// Main text is left-aligned, hint (if any) is right-aligned.
    /// Extra width beyond `self.size.x` goes between the two.
    pub fn text_areas(
        self, rect: egui::Rect, ctx: &egui::Context, clip_rect: egui::Rect,
    ) -> Vec<TextBufferArea> {
        let mut areas = Vec::with_capacity(2);

        let main_rect = egui::Rect::from_min_size(rect.min, self.main.size);
        areas.push(TextBufferArea::new(
            self.main.buffer,
            main_rect,
            self.main.color,
            ctx,
            clip_rect,
        ));

        if let Some(hint) = self.hint {
            let hint_rect = egui::Rect::from_min_size(
                egui::pos2(rect.max.x - hint.size.x, rect.min.y),
                hint.size,
            );
            areas.push(TextBufferArea::new(hint.buffer, hint_rect, hint.color, ctx, clip_rect));
        }

        areas
    }

    /// Convenience for labels without hints — returns a single [`TextBufferArea`].
    pub fn text_area(
        self, rect: egui::Rect, ctx: &egui::Context, clip_rect: egui::Rect,
    ) -> TextBufferArea {
        TextBufferArea::new(self.main.buffer, rect, self.main.color, ctx, clip_rect)
    }
}

impl<'a> GlyphonLabel<'a> {
    /// Plain text label.
    pub fn new(text: &'a str, color: egui::Color32) -> Self {
        Self {
            spans: vec![(text, SpanStyle { bold: false, color: None })],
            color,
            hint: None,
            font_size: 14.0,
            line_height: None,
            max_width: f32::MAX,
            sense: Sense::hover(),
        }
    }

    /// Label with mixed bold/normal spans for search match highlighting.
    /// Each span is `(text, bold)`.
    pub fn new_rich(spans: Vec<(&'a str, bool)>, color: egui::Color32) -> Self {
        Self {
            spans: spans
                .into_iter()
                .map(|(t, bold)| (t, SpanStyle { bold, color: None }))
                .collect(),
            color,
            hint: None,
            font_size: 14.0,
            line_height: None,
            max_width: f32::MAX,
            sense: Sense::hover(),
        }
    }

    /// Label with per-span color overrides. Each span is `(text, optional color)`.
    /// Spans without a color use the base color.
    pub fn new_colored(spans: Vec<(&'a str, Option<egui::Color32>)>, color: egui::Color32) -> Self {
        Self {
            spans: spans
                .into_iter()
                .map(|(t, c)| (t, SpanStyle { bold: false, color: c }))
                .collect(),
            color,
            hint: None,
            font_size: 14.0,
            line_height: None,
            max_width: f32::MAX,
            sense: Sense::hover(),
        }
    }

    /// Adds a right-aligned hint (e.g. a keyboard shortcut) rendered in a
    /// smaller font and the given color.
    pub fn hint(self, text: &'a str, color: egui::Color32) -> Self {
        Self { hint: Some((text, color)), ..self }
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
    pub fn measure(&self, ui: &egui::Ui) -> egui::Vec2 {
        self.build(ui.ctx()).size
    }

    /// Shapes the text and returns the buffers + measured size for manual placement.
    pub fn build(&self, ctx: &egui::Context) -> ShapedLabel {
        let ppi = ctx.pixels_per_point();
        let line_height = self.line_height.unwrap_or(self.font_size * 1.4);

        let main = Self::shape_buffer(
            ctx,
            ppi,
            &self.spans,
            self.color,
            self.font_size,
            line_height,
            self.max_width,
        );

        let hint = self.hint.map(|(text, color)| {
            let hint_font_size = self.font_size - 2.0;
            Self::shape_buffer(
                ctx,
                ppi,
                &[(text, SpanStyle { bold: false, color: None })],
                color,
                hint_font_size,
                line_height,
                f32::MAX,
            )
        });

        let width = match &hint {
            Some(h) => main.size.x + HINT_GAP + h.size.x,
            None => main.size.x,
        };
        let height = match &hint {
            Some(h) => main.size.y.max(h.size.y),
            None => main.size.y,
        };

        ShapedLabel { main, hint, size: egui::vec2(width, height) }
    }

    fn shape_buffer(
        ctx: &egui::Context, ppi: f32, spans: &[(&str, SpanStyle)], color: egui::Color32,
        font_size: f32, line_height: f32, max_width: f32,
    ) -> ShapedBuffer {
        let cache_key = GlyphonCacheKey {
            spans: spans
                .iter()
                .map(|&(text, style)| GlyphonCacheSpan {
                    text: text.to_string(),
                    family: GlyphonFontFamily::SansSerif,
                    bold: style.bold,
                    italic: false,
                    color: style.color.map(|c| c.to_array()),
                })
                .collect(),
            font_size_bits: (font_size * ppi).to_bits(),
            line_height_bits: (line_height * ppi).to_bits(),
            width_bits: (max_width * ppi).to_bits(),
        };

        let font_system = ctx
            .data(|d| d.get_temp::<Arc<Mutex<FontSystem>>>(egui::Id::NULL))
            .expect("cosmic-text font system used before registered");

        let buffer = ctx
            .data(|d| d.get_temp::<Arc<Mutex<GlyphonCache>>>(egui::Id::NULL))
            .expect("glyphon cache used before registered")
            .lock()
            .unwrap()
            .get_or_shape(cache_key, || {
                let mut fs = font_system.lock().unwrap();
                let mut buf =
                    Buffer::new(&mut fs, Metrics::new(font_size * ppi, line_height * ppi));
                buf.set_size(&mut fs, Some(max_width * ppi), None);

                let base = Attrs::new().family(Family::SansSerif);
                buf.set_rich_text(
                    &mut fs,
                    spans.iter().map(|&(text, style)| {
                        let mut attrs = if style.bold {
                            base.clone().weight(Weight::BOLD)
                        } else {
                            base.clone()
                        };
                        if let Some(c) = style.color {
                            attrs = attrs.color(glyphon::Color::rgba(c.r(), c.g(), c.b(), c.a()));
                        }
                        (text, attrs)
                    }),
                    &base,
                    Shaping::Advanced,
                    None,
                );
                buf.shape_until_scroll(&mut fs, false);
                buf
            });

        let buf = buffer.read().unwrap();
        let line_height_px = buf.metrics().line_height;
        let (width, lines) = buf
            .layout_runs()
            .fold((0.0f32, 0u32), |(w, n), r| (w.max(r.line_w), n + 1));
        let size = egui::vec2(width / ppi, lines as f32 * line_height_px / ppi);
        drop(buf);

        let c = color;
        ShapedBuffer { buffer, size, color: glyphon::Color::rgba(c.r(), c.g(), c.b(), c.a()) }
    }
}

impl egui::Widget for GlyphonLabel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let shaped = self.build(ui.ctx());
        let alloc_height = self.line_height.unwrap_or(self.font_size * 1.4);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(shaped.size.x, alloc_height), self.sense);

        if ui.is_rect_visible(rect) {
            let areas = shaped.text_areas(rect, ui.ctx(), rect);
            // Clip the callback rect to the visible viewport. If `rect`
            // is partially scrolled off-screen, `egui_wgpu` clamps it
            // to the screen and a zero-area result drops the callback.
            let callback_rect = rect.intersect(ui.clip_rect());
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    callback_rect,
                    GlyphonRendererCallback::new(areas),
                ));
        }

        response
    }
}
