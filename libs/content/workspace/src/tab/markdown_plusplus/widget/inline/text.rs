use std::mem;

use comrak::nodes::{AstNode, NodeValue, Sourcepos};
use egui::epaint::text::Row;
use egui::text::{CCursor, LayoutJob};
use egui::{Color32, Id, Pos2, Rangef, Sense, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt as _;
use syntect::easy::HighlightLines;

use crate::tab::markdown_plusplus::galleys::GalleyInfo;
use crate::tab::markdown_plusplus::widget::{WrapContext, INLINE_PADDING, ROW_HEIGHT, ROW_SPACING};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn span_text(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext, text: &str) -> f32 {
        let pre_span = self.text_pre_span(node, wrap);
        let mid_span = self.text_mid_span(node, wrap, pre_span, text);
        let post_span = self.text_post_span(node, wrap, pre_span + mid_span);

        pre_span + mid_span + post_span
    }

    /// Show some text. It must not contain newlines. It doesn't matter if it wraps. It doesn't have to be a whole line.
    pub fn show_text(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
        sourcepos: Sourcepos,
    ) {
        let range = self.sourcepos_to_range(sourcepos);
        let text = &self.buffer[range];
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

        wrap.offset += pre_span;

        // syntax highlighting
        let syntax_theme =
            if ui.visuals().dark_mode { &self.syntax_dark_theme } else { &self.syntax_light_theme };
        let mut highlighter = None;
        for ancestor in node.ancestors() {
            match &ancestor.data.borrow().value {
                NodeValue::CodeBlock(code_block) => {
                    if let Some(syntax) = self.syntax_set.find_syntax_by_token(&code_block.info) {
                        highlighter = Some(HighlightLines::new(syntax, syntax_theme));
                    }
                }
                NodeValue::HtmlBlock(_) => {
                    if let Some(syntax) = self.syntax_set.find_syntax_by_token("html") {
                        highlighter = Some(HighlightLines::new(syntax, syntax_theme));
                    }
                }
                _ => {}
            }
        }

        if let Some(highlighter) = highlighter.as_mut() {
            let Ok(regions) = highlighter.highlight_line(text, &self.syntax_set) else {
                return;
            };
            for &(ref style, region) in &regions {
                let text_format = TextFormat {
                    color: Color32::from_rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    ),
                    ..text_format.clone()
                };

                let region_span = self.text_mid_span(node, wrap, Default::default(), region);

                let mut layout_job = LayoutJob::single_section(region.into(), text_format.clone());
                layout_job.wrap.max_width = wrap.width;
                if let Some(first_section) = layout_job.sections.first_mut() {
                    first_section.leading_space = wrap.line_offset();
                }

                let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                let pos = top_left + Vec2::new(0., wrap.line() as f32 * (row_height + ROW_SPACING));

                let mut empty_rows = 0;
                for (i, row) in galley.rows.iter().enumerate() {
                    if row.rect.area() < 1. {
                        empty_rows += 1;
                        continue;
                    }

                    let rect = row.rect.translate(pos.to_vec2());
                    let rect = rect.translate(Vec2::new(
                        0.,
                        i as f32 * ROW_SPACING + empty_rows as f32 * row_height,
                    ));

                    // paint galley row-by-row to take control of row spacing
                    let layout_job = LayoutJob::single_section(row.text(), text_format.clone());
                    let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

                    let byte_range = (galley_start, galley_start + region.len());
                    let range = self.range_to_char(byte_range);
                    let cursor = self.buffer.current.selection.start(); // whatever
                    if range.contains(cursor, true, true) {
                        let egui_cursor = galley.from_ccursor(CCursor {
                            index: (cursor - range.start()).0,
                            prefer_next_row: true,
                        });

                        let max =
                            rect.left_top() + galley.pos_from_cursor(&egui_cursor).max.to_vec2();

                        ui.painter().vline(
                            max.x,
                            Rangef { min: max.y - row_height, max: max.y },
                            egui::Stroke::new(1., self.theme.fg().accent_primary),
                        );
                    }

                    ui.painter()
                        .galley(rect.left_top(), galley.clone(), Default::default());
                    let response = ui.interact(rect, Id::new("galley").with(range), Sense::click());
                    self.galleys.push(GalleyInfo { range, galley, response });

                    // debug
                    // ui.painter().rect_stroke(
                    //     rect,
                    //     2.,
                    //     egui::Stroke::new(1., self.theme.fg().accent_primary),
                    // );
                }

                wrap.offset += region_span;
                galley_start += region.len();
            }
        } else {
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
                if row.rect.area() < 1. {
                    empty_rows += 1;
                    continue;
                }

                let rect = row.rect.translate(pos.to_vec2());
                let rect = rect.translate(Vec2::new(
                    0.,
                    i as f32 * ROW_SPACING + empty_rows as f32 * row_height,
                ));

                if spoiler {
                    if hovered {
                        ui.painter().rect_stroke(
                            rect.expand2(Vec2::new(INLINE_PADDING, 1.)),
                            2.,
                            Stroke::new(1., background),
                        );
                    }
                } else if background != Default::default() {
                    ui.painter().rect(
                        rect.expand2(Vec2::new(INLINE_PADDING, 1.)),
                        2.,
                        background,
                        Stroke::new(1., self.theme.bg().neutral_tertiary),
                    );
                }

                // paint galley row-by-row to take control of row spacing
                let layout_job = LayoutJob::single_section(row.text(), text_format.clone());
                let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));

                if spoiler && !hovered {
                    ui.painter().rect_filled(
                        rect.expand2(Vec2::new(INLINE_PADDING, 1.)),
                        2.,
                        background,
                    );
                }

                let byte_range = (galley_start, galley_start + text.len());
                let range = self.range_to_char(byte_range);
                let cursor = self.buffer.current.selection.start(); // whatever
                if range.contains(cursor, true, true) {
                    let egui_cursor = galley.from_ccursor(CCursor {
                        index: (cursor - range.start()).0,
                        prefer_next_row: true,
                    });

                    let max = rect.left_top() + galley.pos_from_cursor(&egui_cursor).max.to_vec2();

                    ui.painter().vline(
                        max.x,
                        Rangef { min: max.y - row_height, max: max.y },
                        egui::Stroke::new(1., self.theme.fg().accent_primary),
                    );
                }

                ui.painter()
                    .galley(rect.left_top(), galley.clone(), Default::default());
                ui.painter()
                    .hline(rect.x_range(), rect.bottom() - 2.0, underline);
                let response = ui.interact(
                    rect,
                    Id::new("galley").with(self.sourcepos_to_range(sourcepos)),
                    Sense::click(),
                );
                self.galleys.push(GalleyInfo { range, galley, response });

                // debug
                // ui.painter().rect_stroke(
                //     rect,
                //     2.,
                //     egui::Stroke::new(1., self.theme.fg().accent_primary),
                // );
            }

            wrap.offset += mid_span;
            galley_start += text.len();
        }

        // todo: unclear why this isn't needed
        // wrap.offset += post_span;
    }

    fn text_pre_span(&self, node: &AstNode<'_>, wrap: &WrapContext) -> f32 {
        let padded = self.text_format(node).background != Default::default();
        if padded && wrap.line_offset() > 0.5 {
            wrap.line_remaining().min(INLINE_PADDING)
        } else {
            0.
        }
    }

    pub fn text_mid_span(
        &self, node: &'ast AstNode<'ast>, wrap: &WrapContext, pre_span: f32, text: &str,
    ) -> f32 {
        let mut tmp_wrap = WrapContext { offset: wrap.offset + pre_span, ..*wrap };

        // syntax highlighting (it breaks the text into regions which affects how it's wrapped)
        let syntax_theme = &self.syntax_light_theme; // the particular colors don't matter
        let mut highlighter = None;
        for ancestor in node.ancestors() {
            match &ancestor.data.borrow().value {
                NodeValue::CodeBlock(code_block) => {
                    if let Some(syntax) = self.syntax_set.find_syntax_by_token(&code_block.info) {
                        highlighter = Some(HighlightLines::new(syntax, syntax_theme));
                    }
                }
                NodeValue::HtmlBlock(_) => {
                    if let Some(syntax) = self.syntax_set.find_syntax_by_token("html") {
                        highlighter = Some(HighlightLines::new(syntax, syntax_theme));
                    }
                }
                _ => {}
            }
        }

        if let Some(highlighter) = highlighter.as_mut() {
            let Ok(regions) = highlighter.highlight_line(text, &self.syntax_set) else {
                return tmp_wrap.offset - wrap.offset; // intended to be unreachable but not sure
            };
            for &(_, region) in &regions {
                let mut layout_job =
                    LayoutJob::single_section(region.into(), self.text_format(node));
                layout_job.wrap.max_width = wrap.width;
                if let Some(first_section) = layout_job.sections.first_mut() {
                    first_section.leading_space = tmp_wrap.line_offset();
                }

                let galley = self.ctx.fonts(|fonts| fonts.layout_job(layout_job));
                for row in &galley.rows {
                    tmp_wrap.offset += row_span(row, &tmp_wrap);
                }
            }
        } else {
            let mut layout_job = LayoutJob::single_section(text.into(), self.text_format(node));
            layout_job.wrap.max_width = wrap.width;
            if let Some(first_section) = layout_job.sections.first_mut() {
                first_section.leading_space = tmp_wrap.line_offset();
            }

            let galley = self.ctx.fonts(|fonts| fonts.layout_job(layout_job));
            for row in &galley.rows {
                tmp_wrap.offset += row_span(row, &tmp_wrap);
            }
        }

        tmp_wrap.offset - wrap.offset
    }

    fn text_post_span(
        &self, node: &AstNode<'_>, wrap: &WrapContext, pre_plus_mid_span: f32,
    ) -> f32 {
        let padded = self.text_format(node).background != Default::default();
        if padded {
            let wrap = WrapContext { offset: wrap.offset + pre_plus_mid_span, ..*wrap };
            wrap.line_remaining().min(INLINE_PADDING)
        } else {
            0.
        }
    }
}

/// Return the span of the first row, including the remaining space on the previous row if there was one
fn row_span(row: &Row, wrap: &WrapContext) -> f32 {
    row.rect.width() + row_wrap_span(row, wrap).unwrap_or_default()
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
