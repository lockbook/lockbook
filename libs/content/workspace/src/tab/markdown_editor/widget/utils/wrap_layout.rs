use std::mem;
use std::sync::{Arc, RwLock};

use egui::epaint::text::Row;
use egui::text::LayoutJob;
use egui::{Color32, Pos2, Rect, Sense, Stroke, TextFormat, Ui, Vec2};
use egui_wgpu_renderer::egui_wgpu;
use epaint::text::cursor::RCursor;
use glyphon::{Attrs, Family, Metrics, Shaping};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::Lines;
use crate::tab::markdown_editor::galleys::GalleyInfo;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::{INLINE_PADDING, ROW_HEIGHT, ROW_SPACING};
use crate::{GlyphonRendererCallback, TextBufferArea};

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
        &self, wrap: &mut Wrap, range: (DocCharOffset, DocCharOffset), text_format: TextFormat,
    ) -> f32 {
        wrap.offset += self.span_section(wrap, range, text_format);
        wrap.height()
    }

    /// Returns the span of a text section in a wrap layout, which includes
    /// space added to the end of a row when text wraps.
    pub fn span_section(
        &self, wrap: &Wrap, range: (DocCharOffset, DocCharOffset), text_format: TextFormat,
    ) -> f32 {
        self.text_mid_span(wrap, Default::default(), &self.buffer[range], text_format)
    }

    /// Show source text specified by the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    pub fn show_section(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset), text_format: TextFormat, spoiler: bool,
    ) -> Response {
        self.show_override_section(
            ui,
            top_left,
            wrap,
            range,
            text_format,
            spoiler,
            None,
            Sense { click: false, drag: false, focusable: false },
        )
    }

    /// Returns the height of a single section that's not from the document's
    /// source text. Pass a fresh wrap initialized with the desired width.
    pub fn height_override_section(
        &self, wrap: &mut Wrap, text: &str, text_format: TextFormat,
    ) -> f32 {
        wrap.offset += self.span_override_section(wrap, text, text_format);
        wrap.height()
    }

    /// Returns the span of a single section that's not from the document's
    /// source text in a wrap layout, which includes space added to the end of a
    /// row when text wraps.
    pub fn span_override_section(&self, wrap: &Wrap, text: &str, text_format: TextFormat) -> f32 {
        self.text_mid_span(wrap, Default::default(), text, text_format)
    }

    /// Show source text specified by the given range or override text. In the
    /// override case, clicking the text will select the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    #[allow(clippy::too_many_arguments)]
    pub fn show_override_section(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset), mut text_format: TextFormat, spoiler: bool,
        override_text: Option<&str>, sense: Sense,
    ) -> Response {
        let text = override_text.unwrap_or(&self.buffer[range]);
        let underline = mem::take(&mut text_format.underline);
        let background = mem::take(&mut text_format.background);
        let padded = background != Default::default();

        #[cfg(debug_assertions)]
        if text.contains('\n') {
            panic!("show_text_line: text contains newline: {text:?}");
        }

        let pre_span = self.text_pre_span(wrap, text_format.clone());
        let mid_span = self.text_mid_span(wrap, pre_span, text, text_format.clone());
        let post_span = self.text_post_span(wrap, pre_span + mid_span, text_format.clone());

        wrap.offset += pre_span;

        /* begin changeset */

        // todo:
        // 1. first section leading space: shape the buffer with the first row's remaining width
        // 2. determine the rect for each row and draw it, simply as a rect (at first)
        // 3. sort out what's going on with multi-row override text so you don't break something
        // 4. click/hover detection, spoilers, code, highlights
        // 5. rewrite the span fns to match

        // determine the first row
        let mut fs = self.font_system.lock().unwrap();
        let fs = &mut fs;
        let mut buffer =
            glyphon::Buffer::new(fs, Metrics::new(wrap.row_height, wrap.row_height + ROW_SPACING));
        let mut buffers = Vec::new();

        buffer.set_size(fs, Some(wrap.row_remaining()), None);
        buffer.set_text(fs, text, Attrs::new().family(Family::SansSerif), Shaping::Advanced);
        buffer.shape_until_scroll(fs, false);

        let first_row_bytes = if let Some(first_row) = buffer.layout_runs().next() {
            if let Some(last_glyph) = first_row.glyphs.last() { last_glyph.end } else { 0 }
        } else {
            0
        };
        let byte_range = self.range_to_byte(range);
        let first_row_range =
            self.range_to_char((byte_range.start(), byte_range.start() + first_row_bytes));
        let remaining_rows_range = (first_row_range.end(), range.end());

        // layout the first row on its own to continue any existing lines
        buffer.set_size(fs, Some(wrap.row_remaining()), None);
        buffer.set_text(
            fs,
            &self.buffer[first_row_range],
            Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(fs, false);

        if let Some(first_row) = buffer.layout_runs().next() {
            // for glyph in first_row.glyphs {
            //     let mut rect = Rect {
            //         min: Pos2 { x: glyph.x, y: glyph.y },
            //         max: Pos2 { x: glyph.x + glyph.w, y: glyph.y + buffer.metrics().font_size },
            //     };
            //     rect = rect.translate(wrap.offset * Vec2::X);
            //     rect = rect.translate(top_left.to_vec2());

            //     ui.painter().rect_filled(rect, 3., Color32::DARK_GREEN);
            // }

            let buffer_height = buffer
                .layout_runs()
                .last()
                .map(|run| run.line_top + run.line_height)
                .unwrap_or(0.);

            let rect = Rect::from_min_size(top_left, Vec2::new(wrap.width, buffer_height));
            buffers.push(TextBufferArea::new(
                Arc::new(RwLock::new(buffer.clone())),
                rect,
                glyphon::Color::rgb(255, 255, 255),
                ui.ctx(),
            ));
            // ui.painter()
            //     .rect_stroke(rect, 3., egui::Stroke::new(1., egui::Color32::LIGHT_GREEN));
        }

        // layout remaining rows
        buffer.set_size(fs, Some(wrap.width), None);
        buffer.set_text(
            fs,
            &self.buffer[remaining_rows_range],
            Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(fs, false);

        for row in buffer.layout_runs() {
            // for glyph in row.glyphs {
            //     let mut rect = Rect {
            //         min: Pos2 { x: glyph.x, y: glyph.y },
            //         max: Pos2 { x: glyph.x + glyph.w, y: glyph.y + buffer.metrics().font_size },
            //     };
            //     rect = rect.translate(buffer.metrics().line_height * Vec2::Y); // first row
            //     rect = rect.translate(row.line_top * Vec2::Y);
            //     rect = rect.translate(top_left.to_vec2());

            //     ui.painter().rect_filled(rect, 3., Color32::DARK_BLUE);
            // }

            // todo: determine the range for each row; you'll need it for wrap.row_ranges and for galley info
            // this is the place where you sort out what's going on with multi-row override text

            let buffer_height = buffer
                .layout_runs()
                .last()
                .map(|run| run.line_top + run.line_height)
                .unwrap_or(0.);

            let rect = Rect::from_min_size(
                top_left + buffer.metrics().line_height * Vec2::Y,
                Vec2::new(wrap.width, buffer_height),
            );
            buffers.push(TextBufferArea::new(
                Arc::new(RwLock::new(buffer.clone())),
                rect,
                glyphon::Color::rgb(255, 255, 255),
                ui.ctx(),
            ));

            // ui.painter()
            //     .rect_stroke(rect, 3., egui::Stroke::new(1., egui::Color32::LIGHT_BLUE));
        }

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            ui.max_rect(),
            GlyphonRendererCallback { buffers },
        ));

        /* end changeset */

        wrap.offset += post_span;

        Default::default()
    }

    /// Returns the span of pre-text padding for inline code, spoilers, etc.
    pub fn text_pre_span(&self, wrap: &Wrap, text_format: TextFormat) -> f32 {
        let padded = text_format.background != Default::default();
        if padded && wrap.row_offset() > 0.5 {
            INLINE_PADDING.min(wrap.row_remaining())
        } else {
            0.
        }
    }

    /// Returns the span from the text itself of a single section.
    pub fn text_mid_span(
        &self, wrap: &Wrap, pre_span: f32, text: &str, text_format: TextFormat,
    ) -> f32 {
        let mut tmp_wrap =
            Wrap { offset: wrap.offset + pre_span, row_ranges: Default::default(), ..*wrap };

        let text = text.to_string();
        let mut layout_job = LayoutJob::single_section(text, text_format);
        layout_job.wrap.max_width = wrap.width;
        if let Some(first_section) = layout_job.sections.first_mut() {
            first_section.leading_space = tmp_wrap.row_offset();
        }
        layout_job.round_output_size_to_nearest_ui_point = false;

        let galley = self.ctx.fonts(|fonts| fonts.layout_job(layout_job));
        for row in &galley.rows {
            tmp_wrap.offset += row_span(row, &tmp_wrap);
        }

        tmp_wrap.offset - (wrap.offset + pre_span)
    }

    /// Returns the span of post-text padding for inline code, spoilers, etc.
    pub fn text_post_span(
        &self, wrap: &Wrap, pre_plus_mid_span: f32, text_format: TextFormat,
    ) -> f32 {
        let padded = text_format.background != Default::default();
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

/// Return the span of the row, including the remaining space on the previous row if there was one
fn row_span(row: &Row, wrap: &Wrap) -> f32 {
    row_wrap_span(row, wrap).unwrap_or_default() + row.rect.width()
}

/// If the row wrapped, return the remaining space on the line that was ended
fn row_wrap_span(row: &Row, wrap: &Wrap) -> Option<f32> {
    if (row.rect.left() - wrap.row_offset()).abs() > 0.5 {
        Some(wrap.row_remaining())
    } else {
        None
    }
}
