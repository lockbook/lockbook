use std::mem;

use comrak::nodes::{AstNode, NodeValue};
use egui::epaint::text::Row;
use egui::text::LayoutJob;
use egui::{Color32, Pos2, Sense, Stroke, TextFormat, Ui, Vec2};
use syntect::easy::HighlightLines;

use crate::tab::markdown_plusplus::widget::{WrapContext, INLINE_PADDING, ROW_SPACING};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn inline_span_text(&self, node: &AstNode<'_>, wrap: &WrapContext, text: &str) -> f32 {
        let pre_span = self.text_pre_span(node, wrap);
        let mid_span = self.text_mid_span(node, wrap, pre_span, text);
        let post_span = self.text_post_span(node, wrap, pre_span + mid_span);

        pre_span + mid_span + post_span
    }

    pub(crate) fn show_text(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
        text: &str,
    ) {
        let pre_span = self.text_pre_span(node, wrap);
        let mid_span = self.text_mid_span(node, wrap, pre_span, text);
        let post_span = self.text_post_span(node, wrap, pre_span + mid_span);

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

        // most elements will contain only one line, as newline chars are parsed as soft breaks or node boundaries
        // this affects only the few elements that contain multi-text instead of inline children e.g. code blocks
        for line in text.lines() {
            if let Some(highlighter) = highlighter.as_mut() {
                let Ok(regions) = highlighter.highlight_line(line, &self.syntax_set) else {
                    continue;
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

                    let region_span = self.text_mid_span(node, wrap, 0., region);

                    let mut layout_job =
                        LayoutJob::single_section(region.into(), text_format.clone());
                    layout_job.wrap.max_width = wrap.width;
                    if let Some(first_section) = layout_job.sections.first_mut() {
                        first_section.leading_space = wrap.line_offset();
                    }

                    let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                    let pos = top_left
                        + Vec2::new(0., wrap.line() as f32 * (self.row_height(node) + ROW_SPACING));

                    let mut empty_rows = 0;
                    for (i, row) in galley.rows.iter().enumerate() {
                        let rect = row.rect.translate(pos.to_vec2());
                        let rect = rect.translate(Vec2::new(
                            0.,
                            i as f32 * ROW_SPACING + empty_rows as f32 * self.row_height(node),
                        ));

                        if row.rect.area() < 1. {
                            empty_rows += 1;
                            continue;
                        }

                        // paint galley row-by-row to take control of row spacing
                        let layout_job = LayoutJob::single_section(row.text(), text_format.clone());
                        let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                        ui.painter()
                            .galley(rect.left_top(), galley, Default::default());

                        // debug
                        // ui.painter()
                        //     .rect_stroke(rect, 2., egui::Stroke::new(1., self.theme.bg().green));
                    }

                    wrap.offset += region_span;
                }

                wrap.offset = wrap.line_end();
            } else {
                let line_span = self.text_mid_span(node, wrap, Default::default(), line);

                let mut layout_job = LayoutJob::single_section(line.into(), text_format.clone());
                layout_job.wrap.max_width = wrap.width;
                if let Some(first_section) = layout_job.sections.first_mut() {
                    first_section.leading_space = wrap.line_offset();
                }

                let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                let pos = top_left
                    + Vec2::new(0., wrap.line() as f32 * (self.row_height(node) + ROW_SPACING));

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
                        i as f32 * ROW_SPACING + empty_rows as f32 * self.row_height(node),
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
                    ui.painter()
                        .galley(rect.left_top(), galley, Default::default());
                    ui.painter()
                        .hline(rect.x_range(), rect.bottom() - 2.0, underline);

                    if spoiler && !hovered {
                        ui.painter().rect_filled(
                            rect.expand2(Vec2::new(INLINE_PADDING, 1.)),
                            2.,
                            background,
                        );
                    }

                    // debug
                    // ui.painter()
                    //     .rect_stroke(rect, 2., egui::Stroke::new(1., self.theme.bg().blue));
                }

                wrap.offset += line_span;
            }
        }

        if ends_with_multiple_newlines(text) {
            wrap.offset += wrap.width;
        }

        wrap.offset += post_span;
    }

    fn text_pre_span(&self, node: &AstNode<'_>, wrap: &WrapContext) -> f32 {
        let padded = self.text_format(node).background != Default::default();
        if padded && wrap.line_offset() > 0.5 {
            wrap.line_remaining().min(INLINE_PADDING)
        } else {
            0.
        }
    }

    fn text_mid_span(
        &self, node: &AstNode<'_>, wrap: &WrapContext, pre_span: f32, text: &str,
    ) -> f32 {
        println!("vvv  BEGIN text_mid_span: {:?}", text);

        let mut tmp_wrap = WrapContext { offset: wrap.offset + pre_span, ..*wrap };
        for (i, line) in text.lines().enumerate() {
            println!("line: {:?}", line);

            let mut layout_job = LayoutJob::single_section(line.into(), self.text_format(node));
            layout_job.wrap.max_width = wrap.width;
            if let Some(first_section) = layout_job.sections.first_mut() {
                first_section.leading_space = tmp_wrap.line_offset();
            }

            let galley = self.ctx.fonts(|fonts| fonts.layout_job(layout_job));
            for row in &galley.rows {
                println!("  row: {:?}", row.text());
                tmp_wrap.offset += row_span(row, &tmp_wrap);
            }

            if i < text.lines().count() - 1 {
                tmp_wrap.offset = tmp_wrap.line_end();
            }
        }

        if ends_with_multiple_newlines(text) {
            tmp_wrap.offset += wrap.width;
        }

        println!("^^^  END   text_mid_span: {:?}", tmp_wrap.offset - wrap.offset);

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

fn ends_with_multiple_newlines(s: &str) -> bool {
    let mut chars = s.chars().rev();
    let mut newline_count = 0;

    while let Some(c) = chars.next() {
        match c {
            '\n' => newline_count += 1,
            '\r' => {
                if let Some('\n') = chars.next() {
                    // Count CRLF as a single newline
                    newline_count += 1;
                } else {
                    break;
                }
            }
            _ => break,
        }

        if newline_count >= 2 {
            return true;
        }
    }

    false
}
