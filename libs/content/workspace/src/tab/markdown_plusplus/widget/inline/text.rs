use std::mem;

use comrak::nodes::{AstNode, NodeValue, Sourcepos};
use egui::epaint::text::Row;
use egui::text::{CCursor, LayoutJob};
use egui::{Color32, Id, LayerId, Order, Pos2, Rangef, Sense, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_plusplus::galleys::GalleyInfo;
use crate::tab::markdown_plusplus::widget::{WrapContext, INLINE_PADDING, ROW_HEIGHT, ROW_SPACING};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn span_text_line(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext, text: &str) -> f32 {
        let pre_span = self.text_pre_span(node, wrap);
        let mid_span = self.text_mid_span(node, wrap, pre_span, text);
        let post_span = self.text_post_span(node, wrap, pre_span + mid_span);

        // debug
        // println!(
        //     "span_text({:?}); pre_span: {}, mid_span: {}, post_span: {}",
        //     text, pre_span, mid_span, post_span
        // );

        pre_span + mid_span + post_span
    }

    /// Show some text. It must not contain newlines. It doesn't matter if it wraps. It doesn't have to be a whole line.
    pub fn show_text_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
        range: (DocCharOffset, DocCharOffset), color: Option<Color32>,
    ) {
        let text = &self.buffer[range];

        #[cfg(debug_assertions)]
        if text.contains('\n') {
            panic!("show_text_line: text contains newline: {:?}", text);
        }

        let mut galley_start = self.range_to_byte(range).start();

        let pre_span = self.text_pre_span(node, wrap);
        let mid_span = self.text_mid_span(node, wrap, pre_span, text);
        let post_span = self.text_post_span(node, wrap, pre_span + mid_span);

        // todo:
        // * this is a hack to fix line spacing issues with footnote references (mixed font sizes)
        // * using ROW_HEIGHT on its own would neglect headings
        // * footnote references in headings currently look bad
        // * in the target state ROW_HEIGHT is probably not imported at all
        let row_height = self.row_height(node).max(ROW_HEIGHT);

        // we draw the underline & background ourselves
        let mut text_format = self.text_format(node);
        let underline = mem::take(&mut text_format.underline);
        let background = mem::take(&mut text_format.background);
        if let Some(color) = color {
            text_format.color = color;
        }

        wrap.offset += pre_span;

        let mut layout_job = LayoutJob::single_section(text.into(), text_format.clone());
        layout_job.wrap.max_width = wrap.width;
        if let Some(first_section) = layout_job.sections.first_mut() {
            first_section.leading_space = wrap.line_offset();
        }

        let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
        let pos = top_left + Vec2::new(0., wrap.line() as f32 * (row_height + ROW_SPACING));

        let spoiler = node
            .ancestors()
            .any(|node| matches!(node.data.borrow().value, NodeValue::SpoileredText));
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
            let rect = rect
                .translate(Vec2::new(0., i as f32 * ROW_SPACING + empty_rows as f32 * row_height));

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
            let layout_job = LayoutJob::single_section(row.text(), text_format.clone());
            let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

            if spoiler && !hovered {
                ui.painter().rect_filled(expaned_rect, 2., background);
            }

            // debug
            // ui.painter().rect_stroke(
            //     rect,
            //     2.,
            //     egui::Stroke::new(1., self.theme.fg().accent_primary),
            // );

            ui.painter()
                .galley(rect.left_top(), galley.clone(), Default::default());
            ui.painter()
                .hline(rect.x_range(), rect.bottom() - 2.0, underline);

            let byte_range = (galley_start, galley_start + row.text().len());
            let galley_range = self.range_to_char(byte_range);
            let galley_info = GalleyInfo { range: galley_range, galley, rect, padded };

            self.galleys.push(galley_info);

            galley_start += row.text().len();
            if row.rect.area() < 1. {
                empty_rows += 1;
            }
        }

        wrap.offset += mid_span;
        wrap.offset += post_span;
    }

    fn text_pre_span(&self, node: &AstNode<'_>, wrap: &WrapContext) -> f32 {
        let padded = self.text_format(node).background != Default::default();
        if padded && wrap.line_offset() > 0.5 {
            INLINE_PADDING.min(wrap.line_remaining())
        } else {
            0.
        }
    }

    pub fn text_mid_span(
        &self, node: &'ast AstNode<'ast>, wrap: &WrapContext, pre_span: f32, text: &str,
    ) -> f32 {
        let mut tmp_wrap = WrapContext { offset: wrap.offset + pre_span, ..*wrap };

        let mut layout_job = LayoutJob::single_section(text.into(), self.text_format(node));
        layout_job.wrap.max_width = wrap.width;
        if let Some(first_section) = layout_job.sections.first_mut() {
            first_section.leading_space = tmp_wrap.line_offset();
        }

        let galley = self.ctx.fonts(|fonts| fonts.layout_job(layout_job));
        for row in &galley.rows {
            tmp_wrap.offset += row_span(row, &tmp_wrap);
        }

        tmp_wrap.offset - (wrap.offset + pre_span)
    }

    fn text_post_span(
        &self, node: &AstNode<'_>, wrap: &WrapContext, pre_plus_mid_span: f32,
    ) -> f32 {
        let padded = self.text_format(node).background != Default::default();
        if padded {
            let wrap = WrapContext { offset: wrap.offset + pre_plus_mid_span, ..*wrap };
            INLINE_PADDING.min(wrap.line_remaining())
        } else {
            0.
        }
    }
}

/// Return the span of the first row, including the remaining space on the previous row if there was one
fn row_span(row: &Row, wrap: &WrapContext) -> f32 {
    row_wrap_span(row, wrap).unwrap_or_default() + row.rect.width()
}

/// If the row wrapped, return the remaining space on the line that was ended
fn row_wrap_span(row: &Row, wrap: &WrapContext) -> Option<f32> {
    if (row.rect.left() - wrap.line_offset()).abs() > 0.5 {
        Some(wrap.line_remaining())
    } else {
        None
    }
}

pub fn ends_with_newline(s: &str) -> bool {
    s.ends_with('\n') || s.ends_with("\r\n")
}
