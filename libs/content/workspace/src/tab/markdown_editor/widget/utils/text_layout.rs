use std::mem;

use egui::epaint::text::Row;
use egui::text::LayoutJob;
use egui::{Pos2, Sense, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::galleys::GalleyInfo;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::{INLINE_PADDING, ROW_HEIGHT, ROW_SPACING};
use crate::tab::markdown_editor::Editor;

#[derive(Clone, Debug)]
pub struct Wrap {
    pub offset: f32,
    pub width: f32,
    pub row_height: f32, // overridden by headings
}

impl Wrap {
    pub fn new(width: f32) -> Self {
        Self { offset: 0.0, width, row_height: ROW_HEIGHT }
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
        self.show_override_text_line(ui, top_left, wrap, range, text_format, spoiler, None, false)
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
        override_text: Option<&str>, clickable: bool,
    ) -> Response {
        let text = override_text.unwrap_or(&self.buffer[range]);
        let pre_span = self.text_pre_span(wrap, text_format.clone());
        let mid_span = self.text_mid_span(wrap, pre_span, text, text_format.clone());
        let post_span = self.text_post_span(wrap, pre_span + mid_span, text_format.clone());

        wrap.offset += pre_span;

        #[cfg(debug_assertions)]
        if text.contains('\n') {
            panic!("show_text_line: text contains newline: {:?}", text);
        }

        let mut galley_start = self.range_to_byte(range).start();

        let underline = mem::take(&mut text_format.underline);
        let background = mem::take(&mut text_format.background);

        let mut layout_job = LayoutJob::single_section(text.into(), text_format.clone());
        layout_job.wrap.max_width = wrap.width;
        if let Some(first_section) = layout_job.sections.first_mut() {
            first_section.leading_space = wrap.row_offset();
        }

        let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
        let pos = top_left + Vec2::new(0., wrap.row() as f32 * (wrap.row_height + ROW_SPACING));

        let mut hovered = false;
        let mut clicked = false;
        for (i, row) in galley.rows.iter().enumerate() {
            let rect = row.rect.translate(pos.to_vec2());
            let rect = rect.translate(Vec2::new(0., i as f32 * ROW_SPACING));

            let response = ui.allocate_rect(
                rect.expand2(Vec2::new(INLINE_PADDING, 1.)),
                Sense { click: clickable, drag: false, focusable: false },
            );

            hovered |= response.hovered();
            clicked |= response.clicked();
        }

        let mut empty_rows = 0;
        for (i, row) in galley.rows.iter().enumerate() {
            let rect = row.rect.translate(pos.to_vec2());
            let rect = rect.translate(Vec2::new(
                0.,
                i as f32 * ROW_SPACING + empty_rows as f32 * wrap.row_height,
            ));

            let padded = background != Default::default();
            let expanded_rect = if rect.area() < 1. {
                rect
            } else {
                rect.expand2(Vec2::new(INLINE_PADDING - 2., 1.))
            };
            if spoiler {
                if hovered {
                    ui.painter()
                        .rect_stroke(expanded_rect, 2., Stroke::new(1., background));
                }
            } else if padded {
                ui.painter().rect(
                    expanded_rect,
                    2.,
                    background,
                    Stroke::new(1., self.theme.bg().neutral_tertiary),
                );
            }

            // paint galley row-by-row to take control of row spacing
            let text = if text_format.color == self.theme.fg().neutral_quarternary {
                // Replace spaces with dots for whitespace visualization
                row.text().replace(' ', "·") // hack, replicated for span
            } else {
                row.text().to_string()
            };
            let layout_job = LayoutJob::single_section(text, text_format.clone());
            let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

            // debug
            // ui.painter().rect_stroke(
            //     rect,
            //     2.,
            //     egui::Stroke::new(1., text_format.color.gamma_multiply(0.15)),
            // );

            ui.painter()
                .galley(rect.left_top(), galley.clone(), Default::default());
            ui.painter()
                .hline(rect.x_range(), rect.bottom() - 2.0, underline);

            if spoiler && !hovered {
                ui.painter().rect_filled(expanded_rect, 2., background);
            }

            let galley_info = if override_text.is_some() {
                GalleyInfo { range, galley, rect, padded }
            } else {
                let byte_range = (galley_start, galley_start + row.text().len());
                let galley_range = self.range_to_char(byte_range);
                GalleyInfo { range: galley_range, galley, rect, padded }
            };

            self.galleys.push(galley_info);

            galley_start += row.text().len();
            if row.rect.area() < 1. {
                empty_rows += 1;
            }
        }

        wrap.offset += mid_span;
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
        let mut tmp_wrap = Wrap { offset: wrap.offset + pre_span, ..*wrap };

        let text = if text_format.color == self.theme.fg().neutral_quarternary {
            text.replace(' ', "·") // hack, replicated for show
        } else {
            text.to_string()
        };
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
            let wrap = Wrap { offset: wrap.offset + pre_plus_mid_span, ..*wrap };
            INLINE_PADDING.min(wrap.row_remaining())
        } else {
            0.
        }
    }
}

/// Return the span of the first row, including the remaining space on the previous row if there was one
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
