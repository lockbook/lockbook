use egui::{Pos2, Rect, Sense, Ui, Vec2};

use lb_rs::model::text::offset_types::DocCharOffset;

use crate::TextBufferArea;
use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::Lines;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::{INLINE_PADDING, ROW_HEIGHT, ROW_SPACING};

#[derive(Clone, Debug)]
pub struct Wrap {
    pub offset: f32,
    pub width: f32,
    pub row_height: f32, // overridden by headings
    pub row_ranges: Lines,
}

impl Wrap {
    pub fn new(width: f32) -> Self {
        Self { offset: 0.0, width, row_height: ROW_HEIGHT, row_ranges: Vec::new() }
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
        num_rows as f32 * self.row_height + num_spacings as f32 * ROW_SPACING
    }

    pub fn add_range(&mut self, range: (DocCharOffset, DocCharOffset)) {
        let row = self.row();
        if let Some(line) = self.row_ranges.get_mut(row) {
            line.0 = line.0.min(range.0);
            line.1 = line.1.max(range.1);
        } else {
            self.row_ranges.push(range);
        }
    }
}

impl Editor {
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
        self.show_override_section(
            ui,
            top_left,
            wrap,
            range,
            text_format,
            None,
            Sense { click: false, drag: false, focusable: false },
        )
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
        let padded = text_format.background != egui::Color32::TRANSPARENT;

        #[cfg(debug_assertions)]
        if text.contains('\n') {
            panic!("show_text_line: text contains newline: {text:?}");
        }

        let pre_span = self.text_pre_span(wrap, &text_format);
        let mid_span = self.text_mid_span(wrap, pre_span, text, text_format.clone());
        let post_span = self.text_post_span(wrap, pre_span + mid_span, &text_format);

        wrap.offset += pre_span;

        let font_size = wrap.row_height;
        let line_height = wrap.row_height + ROW_SPACING;
        let glyphon_color = {
            let [r, g, b, a] = text_format.color.to_array();
            glyphon::Color::rgba(r, g, b, a)
        };

        // find where the first row breaks by shaping with row remaining width
        let (first_row_text, remaining_text) = {
            let tmp = self.upsert_glyphon_buffer(
                text,
                font_size,
                line_height,
                wrap.row_remaining(),
                &text_format,
            );
            let tmp = tmp.read().unwrap();

            let first_row_bytes = if let Some(first_row) = tmp.layout_runs().next() {
                if let Some(last_glyph) = first_row.glyphs.last() { last_glyph.end } else { 0 }
            } else {
                text.len()
            };
            (text[..first_row_bytes].to_string(), text[first_row_bytes..].to_string())
        };

        // layout the first row
        {
            let row = self.upsert_glyphon_buffer(
                &first_row_text,
                font_size,
                line_height,
                wrap.row_remaining(),
                &text_format,
            );
            let size = row.read().unwrap().shaped_size();
            let pos = top_left + Vec2::new(wrap.row_offset(), wrap.row() as f32 * line_height);
            let rect = Rect::from_min_size(pos, size);

            if ui.clip_rect().intersects(rect) {
                self.text_areas
                    .push(TextBufferArea::new(row, rect, glyphon_color, ui.ctx()));
            }

            if !remaining_text.is_empty() {
                // wrapping row: consume the rest of the row
                wrap.offset += wrap.row_remaining();
            } else {
                // final row: advance by rendered width only
                wrap.offset += size.x;
            }
        }

        // layout the remaining rows
        let tmp = self.upsert_glyphon_buffer(
            &remaining_text,
            font_size,
            line_height,
            wrap.width,
            &text_format,
        );
        let tmp = tmp.read().unwrap();
        let runs_count = tmp.layout_runs().count();
        for (i, run) in tmp.layout_runs().enumerate() {
            let start = run.glyphs.first().map(|g| g.start).unwrap_or(0);
            let end = run.glyphs.last().map(|g| g.end).unwrap_or(0);
            let row_text = if remaining_text[start..].starts_with(' ') {
                &remaining_text[start + 1..end]
            } else {
                &remaining_text[start..end]
            };

            let row = self.upsert_glyphon_buffer(
                row_text,
                font_size,
                line_height,
                wrap.width,
                &text_format,
            );
            let size = row.read().unwrap().shaped_size();
            let pos = top_left + Vec2::new(wrap.row_offset(), wrap.row() as f32 * line_height);
            let rect = Rect::from_min_size(pos, size);

            if ui.clip_rect().intersects(rect) {
                self.text_areas
                    .push(TextBufferArea::new(row, rect, glyphon_color, ui.ctx()));
            }

            if i < runs_count - 1 {
                // wrapping row: consume the rest of the row
                wrap.offset += wrap.row_remaining();
            } else {
                // final row: advance by rendered width only
                wrap.offset += size.x;
            }
        }

        wrap.offset += post_span;

        Default::default()
    }

    /// Returns the span of pre-text padding for inline code, spoilers, etc.
    pub fn text_pre_span(&self, wrap: &Wrap, text_format: &Format) -> f32 {
        let padded = text_format.background != egui::Color32::TRANSPARENT;
        if padded && wrap.row_offset() > 0.5 {
            INLINE_PADDING.min(wrap.row_remaining())
        } else {
            0.
        }
    }

    /// Returns the span from the text itself of a single section.
    pub fn text_mid_span(
        &self, wrap: &Wrap, pre_span: f32, text: &str, text_format: Format,
    ) -> f32 {
        let font_size = wrap.row_height;
        let line_height = wrap.row_height + ROW_SPACING;
        let row_remaining = wrap.row_end() - (wrap.offset + pre_span);

        let (first_row_text, remaining_text) = {
            let tmp = self.upsert_glyphon_buffer(
                text,
                font_size,
                line_height,
                row_remaining,
                &text_format,
            );
            let tmp = tmp.read().unwrap();
            let first_row_bytes = if let Some(first_row) = tmp.layout_runs().next() {
                if let Some(last_glyph) = first_row.glyphs.last() { last_glyph.end } else { 0 }
            } else {
                text.len()
            };
            (text[..first_row_bytes].to_string(), text[first_row_bytes..].to_string())
        };

        // first row
        let first_row_size = {
            let row = self.upsert_glyphon_buffer(
                &first_row_text,
                font_size,
                line_height,
                row_remaining,
                &text_format,
            );
            let guard = row.read().unwrap();
            guard.shaped_size()
        };

        if remaining_text.is_empty() {
            // fits on current row: return rendered width
            first_row_size.x
        } else {
            // wraps: consume rest of current row + remaining rows
            let mut span = row_remaining;

            let tmp = self.upsert_glyphon_buffer(
                &remaining_text,
                font_size,
                line_height,
                wrap.width,
                &text_format,
            );
            let tmp = tmp.read().unwrap();
            let runs_count = tmp.layout_runs().count();
            for (i, run) in tmp.layout_runs().enumerate() {
                let start = run.glyphs.first().map(|g| g.start).unwrap_or(0);
                let end = run.glyphs.last().map(|g| g.end).unwrap_or(0);
                let row_text = if remaining_text[start..].starts_with(' ') {
                    &remaining_text[start + 1..end]
                } else {
                    &remaining_text[start..end]
                };
                let size = self
                    .upsert_glyphon_buffer(
                        row_text,
                        font_size,
                        line_height,
                        wrap.width,
                        &text_format,
                    )
                    .read()
                    .unwrap()
                    .shaped_size();
                if i < runs_count - 1 {
                    // wrapping row: consume the full row
                    span += wrap.width;
                } else {
                    // final row: advance by rendered width only
                    span += size.x;
                }
            }

            span
        }
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
            INLINE_PADDING.min(wrap.row_remaining())
        } else {
            0.
        }
    }
}

trait BufferExt {
    fn shaped_size(&self) -> Vec2;
}

impl BufferExt for glyphon::Buffer {
    fn shaped_size(&self) -> Vec2 {
        let mut result = Vec2::ZERO;
        for run in self.layout_runs() {
            result.y += self.metrics().line_height;
            if let Some(last_glyph) = run.glyphs.last() {
                result.x = result.x.max(last_glyph.x + last_glyph.w)
            }
        }
        result
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
    pub spoiler: bool,
    pub superscript: bool,
    pub subscript: bool,
}
