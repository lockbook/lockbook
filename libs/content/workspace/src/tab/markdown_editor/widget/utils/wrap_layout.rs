use egui::{Pos2, Rect, Sense, Stroke, Ui, Vec2};

use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::TextBufferArea;
use crate::tab::markdown_editor::MdRender;

struct SplitRow {
    text: String,
    range: (DocCharOffset, DocCharOffset),
}
use crate::tab::markdown_editor::bounds::Lines;
use crate::tab::markdown_editor::galleys::GalleyInfo;
use crate::tab::markdown_editor::widget::inline::Response;

#[derive(Clone, Debug)]
pub struct Wrap {
    pub offset: f32,
    pub width: f32,
    pub row_height: f32, // overridden by headings
    pub row_spacing: f32,
    pub row_ranges: Lines,
}

impl Wrap {
    pub fn new(width: f32, row_height: f32, row_spacing: f32) -> Self {
        Self { offset: 0.0, width, row_height, row_spacing, row_ranges: Vec::new() }
    }

    /// The index of the current row
    pub fn row(&self) -> usize {
        (self.offset / self.width) as _
    }

    /// The start of the current row
    pub fn row_start(&self) -> f32 {
        self.row() as f32 * self.width
    }

    /// The end of the current row
    pub fn row_end(&self) -> f32 {
        self.row_start() + self.width
    }

    /// The offset from the start of the row
    pub fn row_offset(&self) -> f32 {
        self.offset - self.row_start()
    }

    /// The remaining space on the row
    pub fn row_remaining(&self) -> f32 {
        self.row_end() - self.offset
    }

    /// The height of the wrapped text; always at least [`Self::row_height`]
    pub fn height(&self) -> f32 {
        let num_rows = ((self.offset / self.width).ceil() as usize).max(1);
        let num_spacings = num_rows.saturating_sub(1);
        num_rows as f32 * self.row_height + num_spacings as f32 * self.row_spacing
    }

    pub fn add_range(&mut self, range: (DocCharOffset, DocCharOffset)) {
        let row = self.row();
        if let Some(line) = self.row_ranges.get_mut(row) {
            line.0 = line.0.min(range.0);
            line.1 = line.1.max(range.1);
        } else {
            self.row_ranges.push(range);

            // prefer next row
            if row > 0 {
                if let Some(prev_line) = self.row_ranges.get_mut(row - 1) {
                    // when two rows' ranges touch, shorten the earlier so that
                    // the boundary belongs to the later
                    if prev_line.end() == range.start() {
                        prev_line.1 -= 1;
                    }
                }
            }
        }
    }
}

impl MdRender {
    pub fn new_wrap(&self, width: f32) -> Wrap {
        Wrap::new(width, self.layout.row_height, self.layout.row_spacing)
    }

    /// Returns the height of a single text section. Pass a fresh wrap initialized with the desired width.
    pub fn height_section(
        &self, wrap: &mut Wrap, range: (DocCharOffset, DocCharOffset), text_format: Format,
    ) -> f32 {
        wrap.offset += self.span_section(wrap, range, text_format);
        wrap.height()
    }

    /// Returns the span of a text section in a wrap layout, which includes
    /// space added to the end of a row when text wraps.
    pub fn span_section(
        &self, wrap: &Wrap, range: (DocCharOffset, DocCharOffset), text_format: Format,
    ) -> f32 {
        self.text_mid_span(wrap, 0., &self.buffer[range], text_format)
    }

    /// Show source text specified by the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    pub fn show_section(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset), text_format: Format,
    ) -> Response {
        self.show_override_section(ui, top_left, wrap, range, text_format, None, Sense::hover())
    }

    /// Returns the height of a single section that's not from the document's
    /// source text. Pass a fresh wrap initialized with the desired width.
    pub fn height_override_section(&self, wrap: &mut Wrap, text: &str, text_format: Format) -> f32 {
        wrap.offset += self.span_override_section(wrap, text, text_format);
        wrap.height()
    }

    /// Returns the span of a single section that's not from the document's
    /// source text in a wrap layout, which includes space added to the end of a
    /// row when text wraps.
    pub fn span_override_section(&self, wrap: &Wrap, text: &str, text_format: Format) -> f32 {
        self.text_mid_span(wrap, 0., text, text_format)
    }

    /// Splits text into rows. For override text all rows share `range`; for
    /// source text each row gets its sub-range (cloned from the buffer).
    fn split_rows(
        &self, override_text: Option<&str>, range: (DocCharOffset, DocCharOffset), font_size: f32,
        wrap: &Wrap, text_format: &Format,
    ) -> Vec<SplitRow> {
        let text = override_text.unwrap_or(&self.buffer[range]);
        let is_override = override_text.is_some();

        let first_row_bytes = {
            let tmp = self.upsert_glyphon_buffer(
                text,
                font_size,
                font_size,
                wrap.row_remaining(),
                text_format,
            );
            let tmp = tmp.read().unwrap();
            tmp.layout_runs()
                .next()
                .and_then(|run| run.glyphs.last())
                .map(|g| g.end)
                .unwrap_or(text.len())
        };

        let first_row_str = text[..first_row_bytes].to_string();
        let remaining_str = text[first_row_bytes..].to_string();

        let first_row_range = if is_override {
            range
        } else {
            let start = self.offset_to_byte(range.start());
            self.range_to_char((start, start + first_row_bytes))
        };

        let mut split = vec![SplitRow { text: first_row_str, range: first_row_range }];

        if !remaining_str.is_empty() {
            let remaining_start_byte = if is_override {
                Default::default() // unused
            } else {
                let remaining_range = (first_row_range.end(), range.end());
                self.range_to_byte(remaining_range).start()
            };

            let run_byte_ranges: Vec<(usize, usize)> = {
                let tmp = self.upsert_glyphon_buffer(
                    &remaining_str,
                    font_size,
                    font_size,
                    wrap.width,
                    text_format,
                );
                let tmp = tmp.read().unwrap();
                tmp.layout_runs()
                    .map(|run| {
                        let start = run.glyphs.first().map(|g| g.start).unwrap_or(0);
                        let end = run.glyphs.last().map(|g| g.end).unwrap_or(0);
                        (start, end)
                    })
                    .collect()
            };

            for (start, end) in run_byte_ranges {
                let skip_leading_space = remaining_str[start..].starts_with(' ');
                let text_start = if skip_leading_space { start + 1 } else { start };
                let row_range = if is_override {
                    range
                } else {
                    self.range_to_char((
                        remaining_start_byte + text_start,
                        remaining_start_byte + end,
                    ))
                };
                split.push(SplitRow {
                    text: remaining_str[text_start..end].to_string(),
                    range: row_range,
                });
            }
        }

        split
    }

    /// Show source text specified by the given range or override text. In the
    /// override case, clicking the text will select the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    #[allow(clippy::too_many_arguments)]
    pub fn show_override_section(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset), text_format: Format, override_text: Option<&str>,
        sense: Sense,
    ) -> Response {
        let text = override_text.unwrap_or(&self.buffer[range]);
        let ppi = self.ctx.pixels_per_point();
        let padded = text_format.background != egui::Color32::TRANSPARENT;
        let sense = if text_format.spoiler { Sense::hover() } else { sense };

        #[cfg(debug_assertions)]
        if text.contains('\n') {
            panic!("show_text_line: text contains newline: {text:?}");
        }

        let pre_span = self.text_pre_span(wrap, &text_format);
        let mid_span = self.text_mid_span(wrap, pre_span, text, text_format.clone());
        let post_span = self.text_post_span(wrap, pre_span + mid_span, &text_format);

        wrap.offset += pre_span;

        let font_size = if text_format.superscript || text_format.subscript {
            wrap.row_height * 0.75
        } else {
            wrap.row_height
        };
        let y_offset = if text_format.subscript { 0.3 * wrap.row_height } else { 0. };
        let color = {
            let [r, g, b, a] = text_format.color.to_array();
            glyphon::Color::rgba(r, g, b, a)
        };

        struct ShapedRow {
            buffer: std::sync::Arc<std::sync::RwLock<glyphon::Buffer>>,
            size: Vec2,
            pos: Pos2,
            rect: Rect,
            range: (DocCharOffset, DocCharOffset),
        }

        // split
        let split = self.split_rows(override_text, range, font_size, wrap, &text_format);
        let split_len = split.len();

        // shape
        let mut shaped: Vec<ShapedRow> = Vec::new();
        for (i, row) in split.into_iter().enumerate() {
            let buffer = self.upsert_glyphon_buffer(
                &row.text,
                font_size,
                font_size,
                wrap.row_remaining(),
                &text_format,
            );
            let size = buffer.read().unwrap().shaped_size(ppi);
            let pos = top_left
                + Vec2::new(
                    wrap.row_offset(),
                    wrap.row() as f32 * (font_size + wrap.row_spacing) + y_offset,
                );
            let rect = Rect::from_min_size(pos, size);
            let advance = if i < split_len - 1 { wrap.row_remaining() } else { size.x };
            wrap.add_range(row.range);
            wrap.offset += advance;
            shaped.push(ShapedRow { buffer, size, pos, rect, range: row.range });
        }

        // sense
        let mut response = Response::default();
        for row in &shaped {
            let interact_rect = row
                .rect
                .expand2(Vec2::new(self.layout.inline_padding, self.layout.row_spacing / 2.));
            let id = ui.id().with((row.pos.x.to_bits(), row.pos.y.to_bits()));
            let egui_resp = ui.interact(interact_rect, id, sense);
            response.hovered |= egui_resp.hovered();
            response.clicked |= egui_resp.clicked();
            if sense == Sense::click() {
                self.touch_consuming_rects.push(interact_rect);
            }
        }

        // draw
        let color = if text_format.spoiler && !response.hovered {
            glyphon::Color::rgba(0, 0, 0, 0)
        } else {
            color
        };
        for row in shaped {
            self.galleys.push(GalleyInfo {
                is_override: override_text.is_some(),
                range: row.range,
                buffer: row.buffer.clone(),
                rect: row.rect,
                padded,
            });
            if ui.clip_rect().intersects(row.rect) {
                self.text_areas.push(TextBufferArea::new(
                    row.buffer,
                    row.rect,
                    color,
                    ui.ctx(),
                    ui.clip_rect(),
                ));
                if row.size.x > 0.001 {
                    self.draw_decorations(
                        ui,
                        row.pos,
                        row.size,
                        font_size,
                        &text_format,
                        response.hovered,
                    );
                }
            }
        }

        wrap.offset += post_span;

        response
    }

    /// Returns the span of pre-text padding for inline code, spoilers, etc.
    pub fn text_pre_span(&self, wrap: &Wrap, text_format: &Format) -> f32 {
        let padded = text_format.background != egui::Color32::TRANSPARENT;
        if padded && wrap.row_offset() > 0.5 {
            self.layout.inline_padding.min(wrap.row_remaining())
        } else {
            0.
        }
    }

    /// Returns the span from the text itself of a single section.
    pub fn text_mid_span(
        &self, wrap: &Wrap, pre_span: f32, text: &str, text_format: Format,
    ) -> f32 {
        let ppi = self.ctx.pixels_per_point();
        let font_size = if text_format.superscript || text_format.subscript {
            wrap.row_height * 0.75
        } else {
            wrap.row_height
        };

        let mut wrap = Wrap { offset: wrap.offset + pre_span, ..wrap.clone() };
        let split = self.split_rows(Some(text), Default::default(), font_size, &wrap, &text_format);
        let split_len = split.len();

        let mut span = 0.0;
        for (i, row) in split.iter().enumerate() {
            let advance = if i < split_len - 1 {
                wrap.row_remaining()
            } else {
                self.upsert_glyphon_buffer(
                    &row.text,
                    font_size,
                    font_size,
                    wrap.row_remaining(),
                    &text_format,
                )
                .read()
                .unwrap()
                .shaped_size(ppi)
                .x
            };
            span += advance;
            wrap.offset += advance;
        }
        span
    }

    /// Returns the span of post-text padding for inline code, spoilers, etc.
    pub fn text_post_span(&self, wrap: &Wrap, pre_plus_mid_span: f32, text_format: &Format) -> f32 {
        let padded = text_format.background != egui::Color32::TRANSPARENT;
        if padded {
            let wrap = Wrap {
                offset: wrap.offset + pre_plus_mid_span,
                row_ranges: Default::default(),
                ..*wrap
            };
            self.layout.inline_padding.min(wrap.row_remaining())
        } else {
            0.
        }
    }

    fn draw_decorations(
        &self, ui: &Ui, pos: Pos2, size: Vec2, font_size: f32, text_format: &Format, hovered: bool,
    ) {
        if text_format.background != egui::Color32::TRANSPARENT {
            let bg_rect =
                Rect::from_min_size(pos, size).expand2(Vec2::splat(self.layout.inline_padding));
            if text_format.spoiler && hovered {
                ui.painter().rect_stroke(
                    bg_rect,
                    2.0,
                    Stroke::new(1.0, text_format.background),
                    egui::epaint::StrokeKind::Inside,
                );
            } else {
                ui.painter().rect(
                    bg_rect,
                    2.0,
                    text_format.background,
                    Stroke::new(1.0, text_format.border),
                    egui::epaint::StrokeKind::Inside,
                );
            }
        }
        let stroke = Stroke::new(1.0, text_format.color);
        let x_range = pos.x..=(pos.x + size.x);
        if text_format.strikethrough {
            ui.painter()
                .hline(x_range.clone(), pos.y + font_size * 0.55, stroke);
        }
        if text_format.underline {
            ui.painter()
                .hline(x_range, pos.y + font_size * 0.95, stroke);
        }
    }
}

pub trait BufferExt {
    fn shaped_size(&self, ppi: f32) -> Vec2;
}

impl BufferExt for glyphon::Buffer {
    fn shaped_size(&self, ppi: f32) -> Vec2 {
        let mut result = Vec2::ZERO;
        for run in self.layout_runs() {
            result.y += self.metrics().line_height;
            if let Some(last_glyph) = run.glyphs.last() {
                result.x = result.x.max(last_glyph.x + last_glyph.w)
            }
        }
        result / ppi
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Sans,
    Mono,
    Icons,
}

#[derive(Clone, Debug)]
pub struct Format {
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
    pub color: egui::Color32,

    pub underline: bool,
    pub strikethrough: bool,
    pub background: egui::Color32,
    pub border: egui::Color32,
    pub spoiler: bool,
    pub superscript: bool,
    pub subscript: bool,
}
