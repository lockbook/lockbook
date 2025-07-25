use std::mem;

use egui::epaint::text::Row;
use egui::text::LayoutJob;
use egui::{Pos2, Rect, Sense, Stroke, TextFormat, Ui, Vec2};
use epaint::text::cursor::RCursor;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::Lines;
use crate::tab::markdown_editor::galleys::GalleyInfo;
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
    /// Kinda hacky. You probably mean to pass a fresh Wrap here.
    pub fn height_text_line(
        &self, wrap: &mut Wrap, range: (DocCharOffset, DocCharOffset), text_format: TextFormat,
    ) -> f32 {
        wrap.offset += self.span_text_line(wrap, range, text_format);
        wrap.height()
    }

    pub fn span_text_line(
        &self, wrap: &Wrap, range: (DocCharOffset, DocCharOffset), text_format: TextFormat,
    ) -> f32 {
        self.text_mid_span(wrap, Default::default(), &self.buffer[range], text_format)
    }

    /// Show the source text specified by the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    pub fn show_text_line(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset), text_format: TextFormat, spoiler: bool,
    ) -> Response {
        self.show_override_text_line(
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

    /// Kinda hacky. You probably mean to pass a fresh Wrap here.
    pub fn height_override_text_line(
        &self, wrap: &mut Wrap, text: &str, text_format: TextFormat,
    ) -> f32 {
        wrap.offset += self.text_mid_span(wrap, Default::default(), text, text_format);
        wrap.height()
    }

    /// Show the source text specified by the given range, optionally overriding
    /// the shown text. In the case of an override, the given range must be
    /// empty, and clicking the text will place the cursor at the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    #[allow(clippy::too_many_arguments)]
    pub fn show_override_text_line(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset), mut text_format: TextFormat, spoiler: bool,
        override_text: Option<&str>, sense: Sense,
    ) -> Response {
        let text = override_text.unwrap_or(&self.buffer[range]);
        let pre_span = self.text_pre_span(wrap, text_format.clone());
        let mid_span = self.text_mid_span(wrap, pre_span, text, text_format.clone());
        let post_span = self.text_post_span(wrap, pre_span + mid_span, text_format.clone());

        wrap.offset += pre_span;

        #[cfg(debug_assertions)]
        if text.contains('\n') {
            panic!("show_text_line: text contains newline: {text:?}");
        }

        let mut galley_start = self.range_to_byte(range).start();

        let underline = mem::take(&mut text_format.underline);
        let background = mem::take(&mut text_format.background);
        let padded = background != Default::default();

        let mut layout_job = LayoutJob::single_section(text.into(), text_format.clone());
        layout_job.wrap.max_width = wrap.width;
        if let Some(first_section) = layout_job.sections.first_mut() {
            first_section.leading_space = wrap.row_offset();
        }

        let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
        let galley_info = GalleyInfo { range, galley, rect: Rect::ZERO, padded: false }; // used for wrap line calculation
        let pos = top_left + Vec2::new(0., wrap.row() as f32 * (wrap.row_height + ROW_SPACING));

        let mut hovered = false;
        let mut clicked = false;
        let mut empty_rows = 0;
        for (i, row) in galley_info.galley.rows.iter().enumerate() {
            let rect = row.rect.translate(pos.to_vec2());
            let rect = rect.translate(Vec2::new(
                0.,
                i as f32 * ROW_SPACING + empty_rows as f32 * wrap.row_height,
            ));

            let response = ui.allocate_rect(rect.expand2(Vec2::new(INLINE_PADDING, 1.)), sense);

            hovered |= response.hovered();
            clicked |= response.clicked();

            if row.rect.area() < 1. {
                empty_rows += 1;
            }
        }

        // break galley into rows to take control of row spacing
        let mut empty_rows = 0;
        for (row_idx, row) in galley_info.galley.rows.iter().enumerate() {
            let row_rect = row.rect.translate(pos.to_vec2());
            let row_rect = row_rect.translate(Vec2::new(
                0.,
                row_idx as f32 * ROW_SPACING + empty_rows as f32 * wrap.row_height,
            ));

            let row_expanded_rect = if row_rect.area() < 1. {
                row_rect
            } else {
                row_rect.expand2(Vec2::new(INLINE_PADDING - 2., 1.))
            };
            if spoiler {
                if hovered {
                    ui.painter()
                        .rect_stroke(row_expanded_rect, 2., Stroke::new(1., background));
                }
            } else if padded {
                ui.painter().rect(
                    row_expanded_rect,
                    2.,
                    background,
                    Stroke::new(1., self.theme.bg().neutral_tertiary),
                );
            }

            let row_text = row.text().to_string();
            let row_layout_job = LayoutJob::single_section(row_text, text_format.clone());
            let row_galley = ui.fonts(|fonts| fonts.layout_job(row_layout_job));

            ui.painter()
                .galley(row_rect.left_top(), row_galley.clone(), Default::default());
            ui.painter()
                .hline(row_rect.x_range(), row_rect.bottom() - 2.0, underline);

            if spoiler && !hovered {
                ui.painter().rect_filled(row_expanded_rect, 2., background);
            }

            let galley_info = if override_text.is_some() {
                GalleyInfo { range, galley: row_galley, rect: row_rect, padded }
            } else {
                let row_galley_byte_range = (galley_start, galley_start + row.text().len());
                let row_galley_range = self.range_to_char(row_galley_byte_range);

                // add the galley range to the wrap to compute wrap line bounds.
                // for override text, only the first row gets ranges, so we
                // don't increment wrap.offset (and therefore the row) until the
                // end of the fn. this may not need to rely on egui cursor math
                // but this is the way it's already known to work.
                wrap.offset += row_span(row, wrap);
                let wrap_line_range = {
                    let start_cursor = galley_info
                        .galley
                        .from_rcursor(RCursor { row: row_idx, column: 0 });
                    let row_start = self
                        .galleys
                        .offset_by_galley_and_cursor(&galley_info, start_cursor);
                    let end_cursor = galley_info.galley.cursor_end_of_row(&start_cursor);
                    let row_end = self
                        .galleys
                        .offset_by_galley_and_cursor(&galley_info, end_cursor);

                    (row_start, row_end).trim(&row_galley_range)
                };
                wrap.add_range(wrap_line_range);

                GalleyInfo { range: wrap_line_range, galley: row_galley, rect: row_rect, padded }
            };

            self.galleys.push(galley_info);

            galley_start += row.text().len();
            if row.rect.area() < 1. {
                empty_rows += 1;
            }
        }

        if override_text.is_some() {
            wrap.offset += mid_span;
        }
        wrap.offset += post_span;

        Response { clicked, hovered }
    }

    pub fn text_pre_span(&self, wrap: &Wrap, text_format: TextFormat) -> f32 {
        let padded = text_format.background != Default::default();
        if padded && wrap.row_offset() > 0.5 {
            INLINE_PADDING.min(wrap.row_remaining())
        } else {
            0.
        }
    }

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

        let galley = self.ctx.fonts(|fonts| fonts.layout_job(layout_job));
        for row in &galley.rows {
            tmp_wrap.offset += row_span(row, &tmp_wrap);
        }

        tmp_wrap.offset - (wrap.offset + pre_span)
    }

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
