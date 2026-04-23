use crate::tab::markdown_editor::{syntax_set, syntax_theme};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt, RangeExt as _, RangeIterExt};
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};

use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn text_format_code_block(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { family: FontFamily::Mono, ..parent_text_format }
    }

    pub fn height_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        if node_code_block.fenced {
            self.height_fenced_code_block(node, node_code_block)
        } else {
            self.height_indented_code_block(node, node_code_block, false)
        }
    }

    pub fn show_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock,
    ) {
        if node_code_block.fenced {
            self.show_fenced_code_block(ui, node, top_left, node_code_block);
        } else {
            self.show_indented_code_block(ui, node, top_left, node_code_block, false);
        }
    }

    pub fn height_fenced_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        let width = self.width(node) - 2. * self.layout.block_padding;

        let mut result = self.layout.block_padding;
        result -= self.layout.row_spacing;

        let reveal = self.reveal_fenced_code_block(node, node_code_block);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            let is_opening_fence = line_idx == first_line_idx;
            let is_closing_fence = !is_opening_fence
                && line_idx == last_line_idx
                && self.is_closing_fence(node, node_code_block, line);

            if is_opening_fence || is_closing_fence {
                if reveal {
                    result += self.layout.row_spacing;
                    result += self.height_section(
                        &mut self.new_wrap(width),
                        node_line,
                        self.text_format_syntax(),
                    );
                }
            } else {
                result += self.layout.row_spacing;
                result += self.height_code_block_line(node, node_code_block, line, false);
            }
        }

        result + self.layout.block_padding
    }

    pub fn show_fenced_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        node_code_block: &NodeCodeBlock,
    ) {
        let mut width = self.width(node);
        let height = self.height_fenced_code_block(node, node_code_block);

        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));
        ui.painter().rect_stroke(
            rect,
            2.,
            Stroke::new(1., self.ctx.get_lb_theme().neutral_bg_tertiary()),
            egui::epaint::StrokeKind::Inside,
        );

        width -= 2. * self.layout.block_padding;
        top_left.x += self.layout.block_padding;
        top_left.y += self.layout.block_padding;
        top_left.y -= self.layout.row_spacing; // makes spacing logic simpler

        let reveal = self.reveal_fenced_code_block(node, node_code_block);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            let is_opening_fence = line_idx == first_line_idx;
            let is_closing_fence = !is_opening_fence
                && line_idx == last_line_idx
                && self.is_closing_fence(node, node_code_block, line);

            if is_opening_fence || is_closing_fence {
                if reveal {
                    top_left.y += self.layout.row_spacing;
                    let mut wrap = self.new_wrap(width);
                    self.show_section(
                        ui,
                        top_left,
                        &mut wrap,
                        node_line,
                        self.text_format_syntax(),
                    );
                    top_left.y += wrap.height();
                    self.bounds.wrap_lines.extend(wrap.row_ranges);
                }
            } else {
                top_left.y += self.layout.row_spacing;
                self.show_code_block_line(ui, node, top_left, node_code_block, line, false);
                top_left.y += self.height_code_block_line(node, node_code_block, line, false);
            }
        }
    }

    fn reveal_fenced_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> bool {
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);

        if first_line_idx == last_line_idx {
            return true; // only opening fence: always reveal
        }
        if first_line_idx + 1 == last_line_idx
            && self.is_closing_fence(node, node_code_block, self.bounds.source_lines[last_line_idx])
        {
            return true; // only opening + closing fence: always reveal
        }

        self.node_lines_intersect_selection(node) // selection-based reveal
    }

    pub fn height_indented_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock, synthetic: bool,
    ) -> f32 {
        let mut result = 0.;

        let reveal = self.reveal_indented_code_block(node, synthetic);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];

            if reveal {
                let node_line = self.node_line(node, line);
                result += self.height_section(
                    &mut self.new_wrap(self.width(node) - 2. * self.layout.block_padding),
                    node_line,
                    self.text_format_syntax(),
                );
            } else {
                result += self.height_code_block_line(node, node_code_block, line, synthetic);
            }

            if line_idx != last_line_idx {
                result += self.layout.row_spacing;
            }
        }

        result + 2. * self.layout.block_padding
    }

    // indented code blocks don't have an info string; the `info` parameter is
    // just used for syntax highlighting when rendering unsupported node types
    // like html blocks as code
    pub fn show_indented_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        node_code_block: &NodeCodeBlock, synthetic: bool,
    ) {
        let width = self.width(node);
        let height = self.height_indented_code_block(node, node_code_block, synthetic);

        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));
        ui.painter().rect_stroke(
            rect,
            2.,
            Stroke::new(1., self.ctx.get_lb_theme().neutral_bg_tertiary()),
            egui::epaint::StrokeKind::Inside,
        );

        top_left.x += self.layout.block_padding;
        top_left.y += self.layout.block_padding;

        let reveal = self.reveal_indented_code_block(node, synthetic);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];

            if reveal {
                let node_line = self.node_line(node, line);
                let mut wrap = self.new_wrap(width);
                self.show_section(ui, top_left, &mut wrap, node_line, self.text_format_syntax());
                top_left.y += wrap.height();
                self.bounds.wrap_lines.extend(wrap.row_ranges);
            } else {
                self.show_code_block_line(ui, node, top_left, node_code_block, line, synthetic);
                top_left.y += self.height_code_block_line(node, node_code_block, line, synthetic);
            }

            top_left.y += self.layout.row_spacing;
        }
    }

    fn reveal_indented_code_block(&self, node: &'ast AstNode<'ast>, synthetic: bool) -> bool {
        let mut reveal = false;
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            // reveal if selection is inside the indentation, but not at the
            // indentation's end / content's start
            let indentation = (
                node_line.start(),
                node_line
                    .end()
                    .min(node_line.start() + if synthetic { 0 } else { 4 }),
            );
            if self.range_revealed(indentation, false)
                || self
                    .reveal_ranges()
                    .any(|rr| rr.contains(indentation.start(), true, true))
            {
                reveal = true;
                break;
            }
        }
        reveal
    }

    fn height_code_block_line(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> f32 {
        let NodeCodeBlock { fenced, fence_offset, info, .. } = node_code_block;

        let node_line = self.node_line(node, line);

        let code_line = if *fenced {
            // "If the leading code fence is indented N spaces, then up to N spaces
            // of indentation are removed from each line of the content (if
            // present). (If a content line is not indented, it is preserved
            // unchanged. If it is indented less than N spaces, all of the
            // indentation is removed.)"
            // https://github.github.com/gfm/#fenced-code-blocks
            let text = &self.buffer[node_line];
            let indentation = text
                .chars()
                .take_while(|&c| c == ' ')
                .count()
                .min(*fence_offset);
            (node_line.start() + indentation, node_line.end())
        } else {
            // "An indented code block is composed of one or more indented chunks
            // separated by blank lines. An indented chunk is a sequence of
            // non-blank lines, each indented four or more spaces. The contents of
            // the code block are the literal contents of the lines, including
            // trailing line endings, minus four spaces of indentation."
            // https://github.github.com/gfm/#indented-code-blocks
            let chunk_start =
                (node_line.start() + if synthetic { 0 } else { 4 }).min(node_line.end());
            (chunk_start, node_line.end())
        };
        let code_line_text = &self.buffer[code_line];

        // syntax highlighting
        let mut highlighter = syntax_set()
            .find_syntax_by_token(info)
            .map(|syntax| HighlightLines::new(syntax, syntax_theme()));

        let mut wrap = self.new_wrap(self.width(node) - 2. * self.layout.block_padding);

        if let Some(highlighter) = highlighter.as_mut() {
            let regions = if let Some(regions) = self.syntax.get(code_line_text, code_line) {
                // cached regions
                regions
            } else {
                // new regions
                let mut regions = Vec::new();
                let mut region_start = self.offset_to_byte(code_line.start());
                for (style, region_str) in highlighter
                    .highlight_line(code_line_text, syntax_set())
                    .unwrap()
                {
                    let region_end = region_start + region_str.len();
                    let region = self.range_to_char((region_start, region_end));
                    regions.push((style, region));
                    region_start = region_end;
                }
                self.syntax
                    .insert(code_line_text.into(), code_line, regions.clone());
                regions
            };

            let text_format = self.text_format(node);
            for (_, region) in regions {
                // color doesn't matter for layout, just how the regions are divided
                wrap.offset += self.span_section(&wrap, region, text_format.clone());
            }
        } else {
            // no syntax highlighting
            wrap.offset += self.span_section(&wrap, code_line, self.text_format(node));
        }

        wrap.height()
    }

    fn show_code_block_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock, line: (Grapheme, Grapheme), synthetic: bool,
    ) {
        let NodeCodeBlock { fenced, fence_offset, info, .. } = node_code_block;

        let node_line = self.node_line(node, line);

        let code_line = if *fenced {
            // "If the leading code fence is indented N spaces, then up to N spaces
            // of indentation are removed from each line of the content (if
            // present). (If a content line is not indented, it is preserved
            // unchanged. If it is indented less than N spaces, all of the
            // indentation is removed.)"
            // https://github.github.com/gfm/#fenced-code-blocks
            let text = &self.buffer[node_line];
            let indentation = text
                .chars()
                .take_while(|&c| c == ' ')
                .count()
                .min(*fence_offset);
            (node_line.start() + indentation, node_line.end())
        } else {
            // "An indented code block is composed of one or more indented chunks
            // separated by blank lines. An indented chunk is a sequence of
            // non-blank lines, each indented four or more spaces. The contents of
            // the code block are the literal contents of the lines, including
            // trailing line endings, minus four spaces of indentation."
            // https://github.github.com/gfm/#indented-code-blocks
            let chunk_start =
                (node_line.start() + if synthetic { 0 } else { 4 }).min(node_line.end());
            (chunk_start, node_line.end())
        };
        let code_line_text = &self.buffer[code_line];

        // syntax highlighting
        let mut highlighter = syntax_set()
            .find_syntax_by_token(info)
            .map(|syntax| HighlightLines::new(syntax, syntax_theme()));

        let mut wrap = self.new_wrap(self.width(node) - 2. * self.layout.block_padding);

        if let Some(highlighter) = highlighter.as_mut() {
            let regions = if let Some(regions) = self.syntax.get(code_line_text, code_line) {
                // cached regions
                regions
            } else {
                // new regions
                let mut regions = Vec::new();
                let mut region_start = self.offset_to_byte(code_line.start());
                for (style, region_str) in highlighter
                    .highlight_line(code_line_text, syntax_set())
                    .unwrap()
                {
                    let region_end = region_start + region_str.len();
                    let region = self.range_to_char((region_start, region_end));
                    regions.push((style, region));
                    region_start = region_end;
                }
                self.syntax
                    .insert(code_line_text.into(), code_line, regions.clone());
                regions
            };

            let mut text_format = self.text_format(node);
            if regions.is_empty() {
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    code_line.start().into_range(),
                    text_format.clone(),
                );
            }
            for (style, region) in regions {
                let hex =
                    Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b)
                        .to_hex();
                let hex = hex.strip_suffix("ff").unwrap();
                text_format.color = self.syntax_color_for_hex(hex);

                self.show_section(ui, top_left, &mut wrap, region, text_format.clone());
            }
        } else {
            // no syntax highlighting
            self.show_section(ui, top_left, &mut wrap, code_line, self.text_format(node));
        }

        self.bounds.wrap_lines.extend(wrap.row_ranges);
    }

    // "The closing code fence may be indented up to three spaces, and may be
    // followed only by spaces, which are ignored."
    // https://github.github.com/gfm/#fenced-code-blocks

    fn is_closing_fence(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme),
    ) -> bool {
        let NodeCodeBlock { fence_char, fence_length, .. } = node_code_block;
        let fence_char = *fence_char as char;

        let node_line = self.node_line(node, line);

        let text = &self.buffer[node_line];
        let mut chars = text.chars().peekable();

        // Skip up to 3 leading spaces
        for _ in 0..3 {
            if chars.peek() == Some(&' ') {
                chars.next();
            } else {
                break;
            }
        }

        // Skip exactly fence_length fence_char's
        for _ in 0..*fence_length {
            if chars.peek() == Some(&fence_char) {
                chars.next();
            } else {
                return false;
            }
        }

        // Skip any number of additional fence_char's
        loop {
            if chars.peek() == Some(&fence_char) {
                chars.next();
            } else {
                break;
            }
        }

        // Any additional characters may be spaces only
        chars.all(|c| c == ' ')
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct SyntaxCacheKey {
    text: String,
    range: (Grapheme, Grapheme),
}

impl SyntaxCacheKey {
    fn new(text: String, range: (Grapheme, Grapheme)) -> Self {
        Self { text, range }
    }
}

pub type SyntaxHighlightResult = Vec<(Style, (Grapheme, Grapheme))>;

#[derive(Clone, Default)]
pub struct SyntaxHighlightCache {
    map: RefCell<HashMap<SyntaxCacheKey, SyntaxHighlightResult>>,
    used_this_frame: RefCell<HashSet<SyntaxCacheKey>>,
}

impl SyntaxHighlightCache {
    pub fn insert(&self, text: String, range: (Grapheme, Grapheme), value: SyntaxHighlightResult) {
        let key = SyntaxCacheKey::new(text, range);
        self.used_this_frame.borrow_mut().insert(key.clone());
        self.map.borrow_mut().insert(key, value);
    }

    pub fn get(&self, text: &str, range: (Grapheme, Grapheme)) -> Option<SyntaxHighlightResult> {
        let key = SyntaxCacheKey::new(text.to_string(), range);
        self.used_this_frame.borrow_mut().insert(key.clone());
        self.map.borrow().get(&key).cloned()
    }

    pub fn garbage_collect(&self) {
        // Remove entries that weren't accessed this frame
        let keys: Vec<SyntaxCacheKey> = self.map.borrow().keys().cloned().collect();
        let used = self.used_this_frame.borrow();
        let mut map = self.map.borrow_mut();
        for key in keys {
            if !used.contains(&key) {
                map.remove(&key);
            }
        }
    }

    pub fn clear(&self) {
        self.map.borrow_mut().clear();
        self.used_this_frame.borrow_mut().clear();
    }
}
