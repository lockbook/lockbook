use std::mem;

use comrak::nodes::{AstNode, NodeValue};
use egui::epaint::text::Row;
use egui::text::LayoutJob;
use egui::{Pos2, Sense, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_plusplus::galleys::GalleyInfo;
use crate::tab::markdown_plusplus::widget::{Wrap, INLINE_PADDING, ROW_SPACING};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn span_node_text_line(&self, node: &'ast AstNode<'ast>, wrap: &Wrap, text: &str) -> f32 {
        let text_format = self.text_format(node);

        let pre_span = self.text_pre_span(wrap, text_format.clone());
        let mid_span = self.text_mid_span(wrap, pre_span, text, text_format.clone());
        let post_span = self.text_post_span(wrap, pre_span + mid_span, text_format);

        pre_span + mid_span + post_span
    }

    pub fn span_text_line(
        &self, wrap: &Wrap, range: (DocCharOffset, DocCharOffset), text_format: TextFormat,
    ) -> f32 {
        self.text_mid_span(wrap, Default::default(), &self.buffer[range], text_format)
    }

    pub fn show_text(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        let sourcepos = node.data.borrow().sourcepos;
        let range = self.sourcepos_to_range(sourcepos);

        self.show_node_text_line(ui, node, top_left, wrap, range)
    }

    /// Show the source text specified by the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    ///
    /// This variant infers the style based on the AST node. It's intended for
    /// the content of the node only, not it's syntax or spacing. For more
    /// control, use the `show_text_line` method.
    pub fn show_node_text_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) {
        let text_format = self.text_format(node);
        let spoiler = node
            .ancestors()
            .any(|node| matches!(node.data.borrow().value, NodeValue::SpoileredText));

        self.show_text_line(ui, top_left, wrap, range, text_format, spoiler);
    }

    /// Show the source text specified by the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    ///
    /// This is the lower-level variant that offers more style control. To infer
    /// the style based on the AST node, use the `show_node_text_line` method.
    #[allow(clippy::too_many_arguments)]
    pub fn show_text_line(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset), text_format: TextFormat, spoiler: bool,
    ) {
        self.show_override_text_line(ui, top_left, wrap, range, text_format, spoiler, None);
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
        override_text: Option<&str>,
    ) {
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
        for (i, row) in galley.rows.iter().enumerate() {
            let rect = row.rect.translate(pos.to_vec2());
            let rect = rect.translate(Vec2::new(0., i as f32 * ROW_SPACING));

            if ui
                .allocate_rect(rect.expand2(Vec2::new(INLINE_PADDING, 1.)), Sense::hover())
                .hovered()
            {
                hovered = true;
            }
        }

        let mut empty_rows = 0;
        for (i, row) in galley.rows.iter().enumerate() {
            let rect = row.rect.translate(pos.to_vec2());
            let rect = rect.translate(Vec2::new(
                0.,
                i as f32 * ROW_SPACING + empty_rows as f32 * wrap.row_height,
            ));

            let padded = background != Default::default();
            let expaned_rect = if rect.area() < 1. {
                rect
            } else {
                rect.expand2(Vec2::new(INLINE_PADDING - 2., 1.))
            };
            if spoiler {
                if hovered {
                    ui.painter()
                        .rect_stroke(expaned_rect, 2., Stroke::new(1., background));
                }
            } else if padded {
                ui.painter().rect(
                    expaned_rect,
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
                ui.painter().rect_filled(expaned_rect, 2., background);
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
    }

    fn text_pre_span(&self, wrap: &Wrap, text_format: TextFormat) -> f32 {
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

    fn text_post_span(&self, wrap: &Wrap, pre_plus_mid_span: f32, text_format: TextFormat) -> f32 {
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

pub fn ends_with_newline(s: &str) -> bool {
    s.ends_with('\n') || s.ends_with("\r\n")
}
