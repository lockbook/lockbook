use std::sync::{Arc, Mutex, RwLock};

use egui::{Response, Sense, Ui};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::TextBufferArea;
use crate::style::TypeRole;
use crate::widgets::glyphon_cache::{
    GlyphonCache, GlyphonCacheKey, GlyphonCacheSpan, GlyphonFontFamily,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextOverflow {
    #[default]
    Clip,
    EndEllipsis,
}

/// Per-span styling: bold flag and optional color override.
#[derive(Clone, Copy, PartialEq)]
pub struct SpanStyle {
    pub bold: bool,
    pub color: Option<egui::Color32>,
    pub mono: bool,
    /// An installed family to draw in instead of the interface's sans.
    pub family: Option<&'static str>,
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
    text_overflow: TextOverflow,
    /// With [`TextOverflow::EndEllipsis`], wrap to this many lines before the
    /// ellipsis instead of one.
    max_lines: usize,
    sense: Sense,
    /// Drawn at [`crate::font::interface_scale`] of `font_size`.
    interface_size: bool,
    /// Lines as tall as the face's ascent and descent, in whole pixels.
    face_lines: bool,
}

/// The result of shaping a [`GlyphonLabel`]. Contains the sized glyphon buffers
/// ready for placement at an arbitrary screen rect.
#[derive(Clone)]
pub struct ShapedLabel {
    main: ShapedBuffer,
    hint: Option<ShapedBuffer>,
    /// Minimum size in logical pixels: main width + gap + hint width.
    pub size: egui::Vec2,
    /// The text was cut short with an ellipsis.
    pub elided: bool,
}

#[derive(Clone)]
struct ShapedBuffer {
    buffer: Arc<RwLock<Buffer>>,
    size: egui::Vec2,
    color: glyphon::Color,
    /// How far down from its box the text draws.
    nudge: f32,
}

/// Gap between main text and hint in logical pixels.
const HINT_GAP: f32 = 8.0;

impl ShapedLabel {
    /// The main text's first baseline, from its top, in logical pixels.
    pub fn baseline(&self, ctx: &egui::Context) -> Option<f32> {
        let buffer = self.main.buffer.read().unwrap();
        let run = buffer.layout_runs().next()?;
        Some(run.line_y / ctx.pixels_per_point())
    }

    fn lines(&self) -> usize {
        self.main.buffer.read().unwrap().layout_runs().count()
    }

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

        let main_rect = egui::Rect::from_min_size(rect.min, self.main.size)
            .translate(egui::vec2(0.0, self.main.nudge));
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

    /// Paints the label with its top left at `pos` on the pixel grid, as egui
    /// places a galley, in `painter`'s layer, clip and opacity, in order among its shapes.
    pub fn paint(self, painter: &egui::Painter, pos: egui::Pos2) {
        use egui::emath::GuiRounding as _;
        if !painter.is_visible() {
            return;
        }
        let clip = painter.clip_rect();
        let pos = pos.round_to_pixels(painter.pixels_per_point());
        let rect = egui::Rect::from_min_size(pos, self.size);
        if !clip.intersects(rect) {
            return;
        }
        let areas = self.text_areas(rect, painter.ctx(), clip);
        crate::paint_text(painter, clip, areas);
    }

    /// [`Self::paint`] with the label's box centered on `center`.
    pub fn paint_centered(self, painter: &egui::Painter, center: egui::Pos2) {
        let pos = center - self.size / 2.0;
        self.paint(painter, pos);
    }

    /// Convenience for labels without hints — returns a single [`TextBufferArea`].
    pub fn text_area(
        self, rect: egui::Rect, ctx: &egui::Context, clip_rect: egui::Rect,
    ) -> TextBufferArea {
        let rect = rect.translate(egui::vec2(0.0, self.main.nudge));
        TextBufferArea::new(self.main.buffer, rect, self.main.color, ctx, clip_rect)
    }
}

impl<'a> GlyphonLabel<'a> {
    /// Plain text label.
    pub fn new(text: &'a str, color: egui::Color32) -> Self {
        Self {
            spans: vec![(text, SpanStyle { bold: false, color: None, mono: false, family: None })],
            color,
            hint: None,
            font_size: 14.0,
            line_height: None,
            max_width: f32::MAX,
            text_overflow: TextOverflow::Clip,
            max_lines: 1,
            sense: Sense::hover(),
            interface_size: false,
            face_lines: false,
        }
    }

    /// A Phosphor icon `size` pt tall, on a line box as tall as the face.
    pub fn icon(glyph: &'a str, color: egui::Color32, size: f32) -> Self {
        Self { face_lines: true, ..Self::new(glyph, color).family("Phosphor").font_size(size) }
    }

    /// Label with mixed bold/normal spans for search match highlighting.
    /// Each span is `(text, bold)`.
    pub fn new_rich(spans: Vec<(&'a str, bool)>, color: egui::Color32) -> Self {
        Self {
            spans: spans
                .into_iter()
                .map(|(t, bold)| (t, SpanStyle { bold, color: None, mono: false, family: None }))
                .collect(),
            color,
            hint: None,
            font_size: 14.0,
            line_height: None,
            max_width: f32::MAX,
            text_overflow: TextOverflow::Clip,
            max_lines: 1,
            sense: Sense::hover(),
            interface_size: false,
            face_lines: false,
        }
    }

    /// Label with per-span color overrides. Each span is `(text, optional color)`.
    /// Spans without a color use the base color.
    pub fn new_colored(spans: Vec<(&'a str, Option<egui::Color32>)>, color: egui::Color32) -> Self {
        Self {
            spans: spans
                .into_iter()
                .map(|(t, c)| (t, SpanStyle { bold: false, color: c, mono: false, family: None }))
                .collect(),
            color,
            hint: None,
            font_size: 14.0,
            line_height: None,
            max_width: f32::MAX,
            text_overflow: TextOverflow::Clip,
            max_lines: 1,
            sense: Sense::hover(),
            interface_size: false,
            face_lines: false,
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

    /// Size, line height and face from the design system's `role`;
    /// [`TypeRole::Mono`] is set in the monospace face.
    pub fn role(self, role: TypeRole) -> Self {
        let label = self.font_size(role.size()).line_height(role.line_height());
        if role == TypeRole::Mono { label.mono() } else { label }
    }

    /// Sets every span in `family`, which [`crate::font::load_family`] must
    /// have added to the font system.
    pub fn family(mut self, family: &'static str) -> Self {
        self.spans
            .iter_mut()
            .for_each(|(_, style)| style.family = Some(family));
        self
    }

    /// Sets every span in the monospace face.
    pub fn mono(mut self) -> Self {
        self.spans
            .iter_mut()
            .for_each(|(_, style)| style.mono = true);
        self
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

    pub fn text_overflow(self, text_overflow: TextOverflow) -> Self {
        Self { text_overflow, ..self }
    }

    pub fn max_lines(self, max_lines: usize) -> Self {
        Self { max_lines: max_lines.max(1), ..self }
    }

    pub fn sense(self, sense: Sense) -> Self {
        Self { sense, ..self }
    }

    /// Draws the text as the interface's other text draws:
    /// [`crate::font::interface_scale`] of its size, on lines as tall as the
    /// face's ascent and descent.
    pub fn interface(self) -> Self {
        Self { interface_size: true, face_lines: true, ..self }
    }

    /// Returns the rendered size in logical pixels without placing the widget.
    pub fn measure(&self, ui: &egui::Ui) -> egui::Vec2 {
        self.build(ui.ctx()).size
    }

    /// Shapes the text and returns the buffers + measured size for manual placement.
    pub fn build(&self, ctx: &egui::Context) -> ShapedLabel {
        if self.interface_size || self.face_lines {
            let mono = self.spans.iter().all(|(_, s)| s.mono);
            let scale = match (self.interface_size, mono) {
                (false, _) => 1.0,
                (true, false) => crate::font::interface_scale(),
                (true, true) => crate::font::interface_scale() * crate::font::MONO_SCALE,
            };
            let font_size = self.font_size * scale;
            let plain =
                Self { font_size, interface_size: false, face_lines: false, ..self.clone_spans() };
            let mut shaped =
                if self.face_lines { plain.build_face_lines(ctx) } else { plain.build(ctx) };
            if self.interface_size && mono {
                shaped.main.nudge = crate::font::MONO_Y_OFFSET * font_size;
            }
            return shaped;
        }
        let ppi = ctx.pixels_per_point();
        let line_height = self.line_height.unwrap_or(self.font_size * 1.4);
        let (spans, elided) = self.overflow_spans(ctx, ppi, line_height);
        let span_refs: Vec<_> = spans
            .iter()
            .map(|span| (span.text.as_str(), span.style))
            .collect();

        let main = Self::shape_buffer(
            ctx,
            ppi,
            &span_refs,
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
                &[(text, SpanStyle { bold: false, color: None, mono: false, family: None })],
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

        ShapedLabel { main, hint, size: egui::vec2(width, height), elided }
    }

    /// Shaped on lines as tall as the face, rounded to whole pixels as egui
    /// rounds a row, the text at the bottom of each.
    fn build_face_lines(&self, ctx: &egui::Context) -> ShapedLabel {
        // one line centers the face in `2 * box - face`, which puts it at the bottom
        let ppi = ctx.pixels_per_point();
        let face = self.face_height(ctx);
        let boxed = (face * ppi).round() / ppi;
        let mut one_line =
            Self { line_height: Some(2.0 * boxed - face), ..self.clone_spans() }.build(ctx);
        if one_line.lines() == 1 {
            one_line.main.size.y = boxed;
            one_line.size.y = boxed;
            return one_line;
        }
        Self { line_height: Some(boxed), ..self.clone_spans() }.build(ctx)
    }

    fn clone_spans(&self) -> Self {
        Self { spans: self.spans.clone(), ..*self }
    }

    /// The first line's ascent plus descent, in logical pixels, or the font
    /// size where it draws nothing.
    fn face_height(&self, ctx: &egui::Context) -> f32 {
        let ppi = ctx.pixels_per_point();
        let shaped = Self::shape_buffer(
            ctx,
            ppi,
            &self.spans,
            self.color,
            self.font_size,
            self.font_size,
            f32::MAX,
        );
        let buffer = shaped.buffer.read().unwrap();
        let line = buffer.lines.first().and_then(|l| l.layout_opt()?.first());
        // a line with no glyphs has no face to measure
        line.map(|l| (l.max_ascent + l.max_descent) / ppi)
            .filter(|h| *h > 0.0)
            .unwrap_or(self.font_size)
    }

    /// The spans as drawn, and whether they were cut short.
    fn overflow_spans(
        &self, ctx: &egui::Context, ppi: f32, line_height: f32,
    ) -> (Vec<OwnedSpan>, bool) {
        if self.text_overflow == TextOverflow::Clip
            || self.max_width == f32::MAX
            || self.max_width <= 0.0
        {
            return (self.owned_spans(), false);
        }

        match self.text_overflow {
            TextOverflow::Clip => (self.owned_spans(), false),
            TextOverflow::EndEllipsis => {
                if self.max_lines > 1 {
                    return self.end_ellipsis_lines(ctx, ppi, line_height);
                }

                let [(text, style)] = self.spans.as_slice() else {
                    return (self.owned_spans(), false);
                };

                if Self::measure_text_width(ctx, ppi, text, *style, self.font_size, line_height)
                    <= self.max_width
                {
                    return (self.owned_spans(), false);
                }

                let text = Self::end_ellipsis(
                    ctx,
                    ppi,
                    text,
                    *style,
                    self.font_size,
                    line_height,
                    self.max_width,
                );
                (vec![OwnedSpan { text, style: *style }], true)
            }
        }
    }

    fn owned_spans(&self) -> Vec<OwnedSpan> {
        self.spans
            .iter()
            .map(|&(text, style)| OwnedSpan { text: text.to_owned(), style })
            .collect()
    }

    fn end_ellipsis(
        ctx: &egui::Context, ppi: f32, text: &str, style: SpanStyle, font_size: f32,
        line_height: f32, max_width: f32,
    ) -> String {
        const ELLIPSIS: &str = "...";

        let ellipsis_width =
            Self::measure_text_width(ctx, ppi, ELLIPSIS, style, font_size, line_height);
        if ellipsis_width > max_width {
            return String::new();
        }

        let chars: Vec<char> = text.chars().collect();

        let mut low = 0;
        let mut high = chars.len();
        while low < high {
            let keep = (low + high).div_ceil(2);
            let candidate = end_ellipsis_candidate(&chars, keep);
            if Self::measure_text_width(ctx, ppi, &candidate, style, font_size, line_height)
                <= max_width
            {
                low = keep;
            } else {
                high = keep - 1;
            }
        }

        let candidate = end_ellipsis_candidate(&chars, low);
        if Self::measure_text_width(ctx, ppi, &candidate, style, font_size, line_height)
            <= max_width
        {
            return candidate;
        }

        ELLIPSIS.to_owned()
    }

    /// The longest start of the spans, ellipsis added when cut, that wraps
    /// to at most `max_lines` lines at `max_width`, and whether it was cut.
    fn end_ellipsis_lines(
        &self, ctx: &egui::Context, ppi: f32, line_height: f32,
    ) -> (Vec<OwnedSpan>, bool) {
        let fits = |candidate: &[OwnedSpan]| {
            let spans: Vec<_> = candidate
                .iter()
                .map(|s| (s.text.as_str(), s.style))
                .collect();
            let shaped = Self::shape_buffer(
                ctx,
                ppi,
                &spans,
                egui::Color32::default(),
                self.font_size,
                line_height,
                self.max_width,
            );
            shaped.size.y <= line_height * self.max_lines as f32 + 0.5
        };
        let whole = self.owned_spans();
        if fits(&whole) {
            return (whole, false);
        }
        let chars: Vec<(char, SpanStyle)> = self
            .spans
            .iter()
            .flat_map(|&(text, style)| text.chars().map(move |c| (c, style)))
            .collect();
        let (mut low, mut high) = (0, chars.len());
        while low < high {
            let keep = (low + high).div_ceil(2);
            if fits(&end_ellipsis_spans(&chars, keep)) {
                low = keep;
            } else {
                high = keep - 1;
            }
        }
        (end_ellipsis_spans(&chars, low), true)
    }

    fn measure_text_width(
        ctx: &egui::Context, ppi: f32, text: &str, style: SpanStyle, font_size: f32,
        line_height: f32,
    ) -> f32 {
        Self::shape_buffer(
            ctx,
            ppi,
            &[(text, style)],
            egui::Color32::default(),
            font_size,
            line_height,
            f32::MAX,
        )
        .size
        .x
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
                    family: match (style.mono, style.family) {
                        (true, _) => GlyphonFontFamily::Monospace,
                        (false, Some(family)) => GlyphonFontFamily::Named(family),
                        (false, None) => GlyphonFontFamily::SansSerif,
                    },
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
                buf.set_size(Some(max_width * ppi), None);

                let base = Attrs::new().family(Family::SansSerif);
                let emoji = Attrs::new().family(Family::Name("Twemoji Mozilla"));
                // Route emoji graphemes to the color font; the rest keep the
                // span's own attrs. Mirrors the editor's `shape_chunk`.
                buf.set_rich_text(
                    spans.iter().flat_map(|&(text, style)| {
                        let mut attrs = match (style.mono, style.family) {
                            (true, _) => base.clone().family(Family::Monospace),
                            (false, Some(family)) => base.clone().family(Family::Name(family)),
                            (false, None) => base.clone(),
                        };
                        if style.bold {
                            attrs = attrs.weight(Weight::BOLD);
                        }
                        if let Some(c) = style.color {
                            attrs = attrs.color(glyphon_color(c));
                        }
                        let emoji = emoji.clone();
                        text.graphemes(true).map(move |g| {
                            if is_emoji_grapheme(g) {
                                (g, emoji.clone())
                            } else if style.family.is_some() {
                                (g, attrs.clone())
                            } else {
                                (g, crate::font::track(attrs.clone(), g, font_size))
                            }
                        })
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

        ShapedBuffer { buffer, size, color: glyphon_color(color), nudge: 0.0 }
    }
}

/// glyphon blends straight alpha; egui's colors are premultiplied.
fn glyphon_color(c: egui::Color32) -> glyphon::Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    glyphon::Color::rgba(r, g, b, a)
}

struct OwnedSpan {
    text: String,
    style: SpanStyle,
}

/// Whether a grapheme should shape against the color emoji font (Twemoji)
/// rather than the surrounding text font — VS-16 presentation sequences and
/// the supplementary-plane emoji blocks. Without this, emoji fall back to the
/// UI font and render as monochrome outlines.
pub(crate) fn is_emoji_grapheme(g: &str) -> bool {
    g.chars().any(|c| {
        matches!(
            c as u32,
            0xFE0F  // variation selector-16: emoji presentation
        | 0x1F000.. // supplementary multilingual plane: core emoji blocks
        )
    })
}

/// The first `keep` styled chars as spans, the ellipsis in the last one's style.
fn end_ellipsis_spans(chars: &[(char, SpanStyle)], keep: usize) -> Vec<OwnedSpan> {
    let mut spans: Vec<OwnedSpan> = vec![];
    for &(c, style) in &chars[..keep] {
        match spans.last_mut() {
            Some(last) if last.style == style => last.text.push(c),
            _ => spans.push(OwnedSpan { text: c.into(), style }),
        }
    }
    match (spans.last_mut(), chars.first()) {
        (Some(last), _) => last.text.push_str("..."),
        (None, Some(&(_, style))) => spans.push(OwnedSpan { text: "...".into(), style }),
        (None, None) => {}
    }
    spans
}

fn end_ellipsis_candidate(chars: &[char], keep: usize) -> String {
    const ELLIPSIS: &str = "...";

    let mut candidate = String::new();
    candidate.extend(chars.iter().take(keep));
    candidate.push_str(ELLIPSIS);
    candidate
}

impl egui::Widget for GlyphonLabel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let shaped = self.build(ui.ctx());
        // Use full shaped size — wrapping (`max_width`) can be multi-line; a
        // single line_height allocation clipped second+ lines (share summary).
        let (rect, response) = ui.allocate_exact_size(shaped.size, self.sense);

        if ui.is_rect_visible(rect) {
            let areas = shaped.text_areas(rect, ui.ctx(), rect);
            // egui_wgpu clamps the callback rect to the screen and drops a zero-area result.
            let callback_rect = rect.intersect(ui.clip_rect());
            crate::paint_text(ui.painter(), callback_rect, areas);
        }

        response
    }
}
