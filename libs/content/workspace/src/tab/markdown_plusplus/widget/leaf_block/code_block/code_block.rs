use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{RangeExt as _, RelByteOffset};
use syntect::easy::HighlightLines;

use crate::tab::markdown_plusplus::{
    bounds::RangesExt as _,
    widget::{inline::text, WrapContext, BLOCK_PADDING, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_code_block(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        let parent_row_height = self
            .ctx
            .fonts(|fonts| fonts.row_height(&parent_text_format.font_id));
        let monospace_row_height = self.ctx.fonts(|fonts| {
            fonts
                .row_height(&FontId { family: FontFamily::Monospace, ..parent_text_format.font_id })
        });
        let monospace_row_height_preserving_size =
            parent_text_format.font_id.size * parent_row_height / monospace_row_height;
        TextFormat {
            font_id: FontId {
                size: monospace_row_height_preserving_size,
                family: FontFamily::Monospace,
            },
            line_height: Some(parent_row_height),
            ..parent_text_format
        }
    }

    pub fn height_code_block(
        &self, node: &'ast AstNode<'ast>, width: f32, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        if node_code_block.fenced {
            self.height_fenced_code_block(node, width, node_code_block)
        } else {
            self.height_indented_code_block(
                node,
                width,
                &node_code_block.info,
                &node_code_block.literal,
            )
        }
    }

    pub fn show_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32,
        node_code_block: &NodeCodeBlock,
    ) {
        if node_code_block.fenced {
            self.show_fenced_code_block(ui, node, top_left, width, node_code_block);
        } else {
            self.show_indented_code_block(
                ui,
                node,
                top_left,
                width,
                Default::default(),
                &node_code_block.literal,
            );
        }
    }

    pub fn height_fenced_code_block(
        &self, node: &'ast AstNode<'ast>, width: f32, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        let code = trim_one_trailing_newline(&node_code_block.literal);
        let text_width = width - 2. * BLOCK_PADDING;

        let info_height = ROW_HEIGHT;
        let code_height = self.text_height(node, &WrapContext::new(text_width), code);
        BLOCK_PADDING + info_height + BLOCK_PADDING + BLOCK_PADDING + code_height + BLOCK_PADDING
    }

    pub fn show_fenced_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32,
        node_code_block: &NodeCodeBlock,
    ) {
        let NodeCodeBlock { fenced: _, fence_char, fence_length, fence_offset, info, literal } =
            node_code_block;
        let fence_length = RelByteOffset(*fence_length);
        let fence_offset = RelByteOffset(*fence_offset);

        let code = trim_one_trailing_newline(literal);
        let text_width = width - 2. * BLOCK_PADDING;

        let info_height = ROW_HEIGHT;
        let code_height =
            self.text_height(node, &WrapContext::new(width - 2. * BLOCK_PADDING), code);
        let height = BLOCK_PADDING
            + info_height
            + BLOCK_PADDING
            + BLOCK_PADDING
            + code_height
            + BLOCK_PADDING;

        // full rect
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));
        ui.painter()
            .rect_stroke(rect, 2., Stroke::new(1., self.theme.bg().neutral_tertiary));

        // info rect
        let info_rect = Rect::from_min_size(
            top_left,
            Vec2::new(width, BLOCK_PADDING + info_height + BLOCK_PADDING),
        );
        ui.painter().rect(
            info_rect,
            2.,
            self.theme.bg().neutral_secondary,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );

        // copy button
        let copy_button_size = ROW_HEIGHT;
        let copy_button_rect = Rect::from_min_size(
            top_left + Vec2::new(text_width - copy_button_size, BLOCK_PADDING),
            Vec2::new(copy_button_size, copy_button_size),
        );
        ui.painter().rect_stroke(
            copy_button_rect,
            2.,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );
        if ui.allocate_rect(copy_button_rect, Sense::click()).clicked() {
            ui.output_mut(|o| o.copied_text = code.into());
        }

        let sourcepos = node.data.borrow().sourcepos;
        let range = self.sourcepos_to_range(sourcepos);
        let start = self.offset_to_byte(range.start());

        // info text
        {
            // "A fenced code block begins with a code fence, indented no more
            // than three spaces. The line with the opening code fence may
            // optionally contain some text following the code fence; this is
            // trimmed of leading and trailing whitespace and called the info
            // string"
            // https://github.github.com/gfm/#fenced-code-blocks

            // we include the leading and trailing whitespace in the editable
            // info string; the trimmed version is still used to determine the
            // language
            // todo: probably want to render it to spec except in some uncapture situation

            let info_line_idx = self
                .bounds
                .source_lines
                .find_containing(range.start(), true, true)
                .start();
            let info_line_range = self.bounds.source_lines[info_line_idx];

            // bounds: add paragraph for info
            let info_start = self.offset_to_char(start + fence_offset + fence_length);
            let info_end = info_line_range.end();
            let info_range = (info_start, info_end);
            self.bounds.paragraphs.push(info_range);

            // draw info
            let info_top_left = top_left + Vec2::splat(BLOCK_PADDING);
            let mut wrap = WrapContext::new(text_width);
            let info_sourcepos = self.range_to_sourcepos(info_range);
            self.show_node_text_line(
                ui,
                node,
                info_top_left,
                &mut wrap,
                self.sourcepos_to_range(info_sourcepos),
            );
        }

        // code text
        {
            let code_top_left = top_left
                + Vec2::new(
                    BLOCK_PADDING,
                    BLOCK_PADDING + info_height + BLOCK_PADDING + BLOCK_PADDING,
                );
            let mut wrap = WrapContext::new(text_width);

            let info_line_idx = sourcepos.start.line - 1; // convert cardinal to ordinal
            let mut code_line_idx = info_line_idx + 1;

            // "If the end of the containing block (or document) is reached and
            // no closing code fence has been found, the code block contains all
            // of the lines after the opening code fence until the end of the
            // containing block (or document)."
            // https://github.github.com/gfm/#fenced-code-blocks
            let last_line_idx = sourcepos.end.line - 1; // convert cardinal to ordinal
            let last_line_range = self.bounds.source_lines[last_line_idx];
            let last_line = &self.buffer[last_line_range];
            let code_block_closed = is_closing_fence(last_line, fence_char, fence_length.0);
            let last_code_line_idx =
                if code_block_closed { last_line_idx - 1 } else { last_line_idx };
            while code_line_idx <= last_code_line_idx {
                // "If the leading code fence is indented N spaces, then up to N
                // spaces of indentation are removed from each line of the
                // content (if present). (If a content line is not indented, it
                // is preserved unchanged. If it is indented less than N spaces,
                // all of the indentation is removed.)"
                // https://github.github.com/gfm/#fenced-code-blocks
                let code_line_range = self.bounds.source_lines[code_line_idx];
                let code_line_text = &self.buffer[code_line_range];

                let code_line_indentation_spaces = code_line_text
                    .chars()
                    .take_while(|&c| c == ' ')
                    .count()
                    .min(fence_offset.0);
                let code_range =
                    (code_line_range.start() - code_line_indentation_spaces, code_line_range.end());
                let code_sourcepos = self.range_to_sourcepos(code_range);

                // bounds
                self.bounds.paragraphs.push(code_range);

                // syntax highlighting
                let syntax_theme = if ui.visuals().dark_mode {
                    &self.syntax_dark_theme
                } else {
                    &self.syntax_light_theme
                };
                let mut highlighter = self
                    .syntax_set
                    .find_syntax_by_token(info)
                    .map(|syntax| HighlightLines::new(syntax, syntax_theme));

                // show text
                if let Some(highlighter) = highlighter.as_mut() {
                    // highlighted text shown as individual regions
                    let regions = if let Some(regions) = self.syntax.get(code_line_text) {
                        regions.clone()
                    } else {
                        highlighter
                            .highlight_line(code_line_text, &self.syntax_set)
                            .unwrap()
                            .into_iter()
                            .map(|(style, region)| (style, region.to_string()))
                            .collect::<Vec<_>>()
                    };

                    let mut region_info = Vec::new();
                    let mut region_start = self.offset_to_byte(code_range.start());
                    for (style, region) in regions {
                        let region_end = region_start + region.len();
                        let region_range = self.range_to_char((region_start, region_end));
                        region_info.push((
                            self.range_to_sourcepos(region_range),
                            Color32::from_rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ),
                        ));
                        region_start = region_end;
                    }

                    for (sourcepos, color) in region_info {
                        let mut text_format = self.text_format(node);
                        text_format.color = color;
                        self.show_text_line(
                            ui,
                            code_top_left,
                            &mut wrap,
                            self.sourcepos_to_range(sourcepos),
                            text_format,
                            false,
                        );
                    }
                } else {
                    self.show_node_text_line(
                        ui,
                        node,
                        code_top_left,
                        &mut wrap,
                        self.sourcepos_to_range(code_sourcepos),
                    );
                }

                // all lines except the last one end in a newline...
                if code_line_idx < last_code_line_idx {
                    wrap.offset = wrap.line_end();
                }

                code_line_idx += 1;
            }

            // ...and sometimes the last one also ends with a newline
            if text::ends_with_newline(code) {
                wrap.offset = wrap.line_end();
            }
        }
    }

    pub fn height_indented_code_block(
        &self, node: &'ast AstNode<'ast>, width: f32, info: &str, code: &str,
    ) -> f32 {
        todo!()
    }

    // indented code blocks don't have an info string; the `info` parameter is
    // just used for syntax highlighting when rendering unsupported node types
    // like html blocks as code
    pub fn show_indented_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32, info: &str,
        code: &str,
    ) {
        todo!()
    }
}

fn trim_one_trailing_newline(code: &str) -> &str {
    code.strip_suffix("\r\n")
        .or_else(|| code.strip_suffix('\n'))
        .unwrap_or(code)
}

// "The closing code fence may be indented up to three spaces, and may be
// followed only by spaces, which are ignored."
// https://github.github.com/gfm/#fenced-code-blocks
fn is_closing_fence(line: &str, fence_char: &u8, fence_length: usize) -> bool {
    let ch = *fence_char as char;
    let s = line.trim_end(); // Remove trailing spaces

    let mut chars = s.chars();

    // Skip up to 3 leading spaces
    for _ in 0..3 {
        if chars.clone().next() == Some(' ') {
            chars.next();
        } else {
            break;
        }
    }

    // Must have at least fence_length fence characters
    let mut count = 0;
    while let Some(c) = chars.clone().next() {
        if c == ch {
            count += 1;
            chars.next();
        } else {
            if count < fence_length {
                return false;
            }
            break;
        }
    }

    // Any additional characters may be spaces only
    chars.all(|c| c == ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_closing_fence() {
        assert!(is_closing_fence("```", &b'`', 3));
        assert!(is_closing_fence("~~~", &b'~', 3));
        assert!(is_closing_fence("```                    ", &b'`', 3)); // any number of trailing spaces is ok
        assert!(is_closing_fence("   ```", &b'`', 3)); // up to 3 leading spaces is ok

        assert!(!is_closing_fence("```", &b'~', 3)); // fence char mismatch
        assert!(!is_closing_fence("~~~", &b'`', 3)); // fence char mismatch
        assert!(!is_closing_fence("```", &b'~', 4)); // not enough fence chars
        assert!(!is_closing_fence("    ```", &b'~', 4)); // too much leading space
        assert!(!is_closing_fence("```   #", &b'~', 4)); // trailing character that isn't a space
    }
}
