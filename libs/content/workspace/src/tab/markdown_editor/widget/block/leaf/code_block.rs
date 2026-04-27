use crate::tab::markdown_editor::{syntax_set, syntax_theme};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt, RangeExt as _, RangeIterExt};
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;

use comrak::nodes::NodeValue;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::consume_indent_columns;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};

use crate::theme::palette_v2::ThemeExt as _;

/// Language tokens whose bundled syntect grammar panics inside
/// `highlight_line` (lazy regex compile failures with fancy-regex).
/// Skip highlighting upfront — there's no per-line panic safety net,
/// so an unlisted bad grammar will crash the renderer.
const SKIP_HIGHLIGHT_TOKENS: &[&str] = &[
    // "JavaScript (Babel)" uses `\g` regex backref that fancy-regex
    // can't compile. Panics with `ParseError(InvalidEscape("\\g"))`.
    "js",
    "javascript",
];

fn should_skip_highlight(info: &str) -> bool {
    SKIP_HIGHLIGHT_TOKENS
        .iter()
        .any(|t| t.eq_ignore_ascii_case(info))
}

impl<'ast> MdRender {
    pub fn text_format_code_block(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { family: FontFamily::Mono, ..parent_text_format }
    }

    /// Paint the code block chrome (border) into `rect`. Same for
    /// fenced and indented blocks.
    fn chrome_code_block(&self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_stroke(
            rect,
            2.,
            Stroke::new(1., self.ctx.get_lb_theme().neutral_bg_tertiary()),
            egui::epaint::StrokeKind::Inside,
        );
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

    /// Precise per-line height: mirrors `show_code_block_line`'s shape
    /// calls (same indent strip, same syntax-region chunking) but
    /// advances the wrap via `span_section` instead of painting via
    /// `show_section`. Same content + same chunking → same cosmic-text
    /// wrap → same `wrap.height()` as what show paints.
    fn height_code_block_line(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> f32 {
        let (code_line, regions) =
            self.code_block_line_chunks(node, node_code_block, line, synthetic);
        let mut wrap = self.new_wrap(self.width(node) - 2. * self.layout.block_padding);
        let text_format = self.text_format(node);
        if let Some(regions) = regions {
            if regions.is_empty() {
                wrap.offset +=
                    self.span_section(&wrap, code_line.start().into_range(), text_format.clone());
            }
            for (_style, region) in regions {
                wrap.offset += self.span_section(&wrap, region, text_format.clone());
            }
        } else {
            wrap.offset += self.span_section(&wrap, code_line, text_format);
        }
        wrap.height()
    }

    /// Shared chunking logic used by both `height_code_block_line` and
    /// `show_code_block_line` so they produce identical wrap geometry.
    /// Returns the indent-stripped line range and (when syntax
    /// highlighting applies) the per-region breakdown.
    fn code_block_line_chunks(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> CodeBlockLineChunks {
        let NodeCodeBlock { fenced, fence_offset, info, .. } = node_code_block;
        let node_line = self.node_line(node, line);
        let code_line = if *fenced {
            let text = &self.buffer[node_line];
            let indentation = text
                .chars()
                .take_while(|&c| c == ' ')
                .count()
                .min(*fence_offset);
            (node_line.start() + indentation, node_line.end())
        } else {
            let target_cols = if synthetic {
                0
            } else {
                let parent_item_padding = node
                    .ancestors()
                    .find_map(|a| match &a.data.borrow().value {
                        NodeValue::Item(nl) => Some(nl.padding),
                        _ => None,
                    })
                    .unwrap_or(0);
                parent_item_padding + 4
            };
            let text = &self.buffer[node_line];
            let strip_graphemes = consume_indent_columns(text, target_cols);
            let chunk_start = (node_line.start() + strip_graphemes).min(node_line.end());
            (chunk_start, node_line.end())
        };
        let code_line_text = &self.buffer[code_line];
        let mut highlighter = if should_skip_highlight(info) {
            None
        } else {
            syntax_set()
                .find_syntax_by_token(info)
                .map(|syntax| HighlightLines::new(syntax, syntax_theme()))
        };
        let regions = highlighter.as_mut().and_then(|h| {
            self.syntax.get(code_line_text, code_line).or_else(|| {
                let line_start = self.offset_to_byte(code_line.start());
                let highlighted = h.highlight_line(code_line_text, syntax_set()).ok()?;
                let mut regions = Vec::new();
                let mut region_start = line_start;
                for (style, region_str) in highlighted {
                    let region_end = region_start + region_str.len();
                    let region = self.range_to_char((region_start, region_end));
                    regions.push((style, region));
                    region_start = region_end;
                }
                self.syntax
                    .insert(code_line_text.into(), code_line, regions.clone());
                Some(regions)
            })
        });
        (code_line, regions)
    }

    pub fn show_fenced_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        node_code_block: &NodeCodeBlock,
    ) {
        let mut width = self.width(node);
        let height = self.height_fenced_code_block(node, node_code_block);
        self.chrome_code_block(ui, Rect::from_min_size(top_left, Vec2::new(width, height)));

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

            let width = self.width(node) - 2. * self.layout.block_padding;
            if reveal {
                let node_line = self.node_line(node, line);
                result += self.height_section(
                    &mut self.new_wrap(width),
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
        self.chrome_code_block(ui, Rect::from_min_size(top_left, Vec2::new(width, height)));

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

    fn show_code_block_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock, line: (Grapheme, Grapheme), synthetic: bool,
    ) {
        let (code_line, regions) =
            self.code_block_line_chunks(node, node_code_block, line, synthetic);
        let mut wrap = self.new_wrap(self.width(node) - 2. * self.layout.block_padding);
        if let Some(regions) = regions {
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

pub type SyntaxHighlightResult = Vec<(Style, (Grapheme, Grapheme))>;

/// Output of [`MdRender::code_block_line_chunks`]: the indent-stripped
/// line range plus optional syntax-highlighted regions.
type CodeBlockLineChunks = ((Grapheme, Grapheme), Option<SyntaxHighlightResult>);

#[derive(Clone)]
pub struct SyntaxHighlightCache {
    map: RefCell<HashMap<SyntaxCacheKey, SyntaxHighlightResult>>,
    /// Hashes of keys touched this frame. `u64` instead of full keys
    /// avoids per-lookup `String` clones for the bookkeeping side.
    /// Hashes are produced by `hasher` so they're consistent within
    /// the lifetime of this cache.
    used_this_frame: RefCell<HashSet<u64>>,
    hasher: std::collections::hash_map::RandomState,
}

impl Default for SyntaxHighlightCache {
    fn default() -> Self {
        Self {
            map: RefCell::default(),
            used_this_frame: RefCell::default(),
            hasher: std::collections::hash_map::RandomState::new(),
        }
    }
}

impl SyntaxHighlightCache {
    pub fn insert(&self, text: String, range: (Grapheme, Grapheme), value: SyntaxHighlightResult) {
        let key = SyntaxCacheKey { text, range };
        self.used_this_frame
            .borrow_mut()
            .insert(self.hash_key(&key));
        self.map.borrow_mut().insert(key, value);
    }

    pub fn get(&self, text: &str, range: (Grapheme, Grapheme)) -> Option<SyntaxHighlightResult> {
        // Mark the lookup hash as used without cloning the text twice
        // (insert into the HashSet would otherwise need a full key).
        // The map lookup itself still needs an owned key since std
        // HashMap doesn't expose raw_entry stably.
        let key = SyntaxCacheKey { text: text.to_string(), range };
        self.used_this_frame
            .borrow_mut()
            .insert(self.hash_key(&key));
        self.map.borrow().get(&key).cloned()
    }

    pub fn garbage_collect(&self) {
        let used = self.used_this_frame.borrow();
        self.map
            .borrow_mut()
            .retain(|key, _| used.contains(&self.hash_key(key)));
        drop(used);
        self.used_this_frame.borrow_mut().clear();
    }

    pub fn clear(&self) {
        self.map.borrow_mut().clear();
        self.used_this_frame.borrow_mut().clear();
    }

    fn hash_key(&self, key: &SyntaxCacheKey) -> u64 {
        self.hasher.hash_one(key)
    }
}
