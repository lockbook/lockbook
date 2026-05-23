use crate::tab::markdown_editor::{syntax_set, syntax_theme};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt};
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;

use comrak::nodes::NodeValue;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::consume_indent_columns;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format, Layout};

use crate::theme::palette_v2::ThemeExt as _;

/// Language tokens whose bundled syntect grammar panics inside
/// `highlight_line` — fancy-regex can't compile features the
/// grammars use (`\g` backrefs, non-constant lookbehinds, escape
/// sequences inside character classes).
const SKIP_HIGHLIGHT_TOKENS: &[&str] = &[
    // ARM Assembly. `s` is the file extension, so a user typing the
    // opening fence `` ``` `` then `s` momentarily fences an ARM
    // Assembly block.
    "s",
    // Command Help
    "cmd-help",
    "help",
    // JavaScript (Babel)
    "js",
    "javascript",
    "mjs",
    "jsx",
    "babel",
    "es6",
    "cjs",
    // JavaScript (Rails)
    "js.erb",
    // LiveScript
    "ls",
    "Slakefile",
    "ls.erb",
    // PowerShell
    "ps1",
    "psm1",
    "psd1",
    // QML
    "qml",
    "qmlproject",
    // Regular Expressions (Elixir)
    "ex.re",
    // SCSS / Sass
    "scss",
    "sass",
    // Salt State (SLS)
    "sls",
    // VimHelp
    "vimhelp",
];

pub fn should_skip_highlight(info: &str) -> bool {
    SKIP_HIGHLIGHT_TOKENS
        .iter()
        .any(|t| t.eq_ignore_ascii_case(info))
}

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
        let row_height = self.layout.row_height;

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
                    let fence_layout = self.compute_section_layout_new(
                        node_line,
                        width,
                        row_height,
                        self.text_format_syntax(),
                    );
                    result += fence_layout.height;
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
        let row_height = self.layout.row_height;

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
                    let fence_layout = self.compute_section_layout_new(
                        node_line,
                        width,
                        row_height,
                        self.text_format_syntax(),
                    );
                    let h = fence_layout.height;
                    self.show_wrap_layout(ui, top_left, &fence_layout);
                    top_left.y += h;
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
        let width = self.width(node) - 2. * self.layout.block_padding;
        let row_height = self.layout.row_height;

        let reveal = self.reveal_indented_code_block(node, synthetic);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];

            if reveal {
                let node_line = self.node_line(node, line);
                let l = self.compute_section_layout_new(
                    node_line,
                    width,
                    row_height,
                    self.text_format_syntax(),
                );
                result += l.height;
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
        let inner_width = width - 2. * self.layout.block_padding;
        let row_height = self.layout.row_height;
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
                let l = self.compute_section_layout_new(
                    node_line,
                    inner_width,
                    row_height,
                    self.text_format_syntax(),
                );
                let h = l.height;
                self.show_wrap_layout(ui, top_left, &l);
                top_left.y += h;
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

    /// Compute the `(chunk_start, end)` slice of `node_line` that's the
    /// actual content of a code-block line (fence indent stripped for
    /// fenced; 4-col indent stripped for indented, combined with parent
    /// item padding if any).
    fn code_line_range(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        node_line: (Grapheme, Grapheme), synthetic: bool,
    ) -> (Grapheme, Grapheme) {
        let NodeCodeBlock { fenced, fence_offset, .. } = node_code_block;
        if *fenced {
            // "If the leading code fence is indented N spaces, then up to N spaces
            // of indentation are removed from each line of the content..."
            // https://github.github.com/gfm/#fenced-code-blocks
            let text = &self.buffer[node_line];
            let indentation = text
                .chars()
                .take_while(|&c| c == ' ')
                .count()
                .min(*fence_offset);
            (node_line.start() + indentation, node_line.end())
        } else {
            // "An indented code block ... minus four spaces of indentation."
            // https://github.github.com/gfm/#indented-code-blocks
            //
            // Strip column-aware. If inside a list item, combine with
            // the item's padding so a tab straddling the boundary
            // doesn't get attributed entirely to one side.
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
        }
    }

    /// Compute syntax-highlighted regions for a code line, or `None`
    /// if no highlighter applies. Caches into `self.syntax`.
    fn code_line_regions(
        &self, info: &str, code_line: (Grapheme, Grapheme), code_line_text: &str,
    ) -> Option<SyntaxHighlightResult> {
        if should_skip_highlight(info) {
            return None;
        }
        let mut highlighter = syntax_set()
            .find_syntax_by_token(info)
            .map(|syntax| HighlightLines::new(syntax, syntax_theme()))?;
        self.syntax.get(code_line_text, code_line).or_else(|| {
            let line_start = self.offset_to_byte(code_line.start());
            let highlighted = highlighter
                .highlight_line(code_line_text, syntax_set())
                .ok()?;
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
    }

    /// Build a `Layout` for one code-block line. Each highlighted
    /// region becomes its own `push_source` with a per-region color
    /// (mono font + colored). Without highlighting, the whole line
    /// goes in as one push_source with the default code format.
    fn layout_code_block_line(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> Layout {
        let node_line = self.node_line(node, line);
        let code_line = self.code_line_range(node, node_code_block, node_line, synthetic);
        let code_line_text = &self.buffer[code_line];
        let mut layout = Layout::new(code_line);
        let regions = self.code_line_regions(&node_code_block.info, code_line, code_line_text);
        if let Some(regions) = regions {
            let mut text_format = self.text_format(node);
            for (style, region) in regions {
                let hex =
                    Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b)
                        .to_hex();
                let hex = hex.strip_suffix("ff").unwrap();
                text_format.color = self.syntax_color_for_hex(hex);
                if !region.is_empty() {
                    layout.push_source(region, &self.buffer[region], text_format.clone());
                }
            }
        } else if !code_line.is_empty() {
            layout.push_source(code_line, code_line_text, self.text_format(node));
        }
        layout
    }

    fn height_code_block_line(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> f32 {
        let width = self.width(node) - 2. * self.layout.block_padding;
        let layout = self.layout_code_block_line(node, node_code_block, line, synthetic);
        self.compute_layout_from(layout, width, self.layout.row_height)
            .height
    }

    fn show_code_block_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock, line: (Grapheme, Grapheme), synthetic: bool,
    ) {
        let width = self.width(node) - 2. * self.layout.block_padding;
        let layout = self.layout_code_block_line(node, node_code_block, line, synthetic);
        let result = self.compute_layout_from(layout, width, self.layout.row_height);
        self.show_wrap_layout(ui, top_left, &result);
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

#[derive(Clone)]
pub struct SyntaxHighlightCache {
    map: RefCell<HashMap<SyntaxCacheKey, SyntaxHighlightResult>>,
    /// Hashes of keys touched this frame. `u64` instead of full keys
    /// avoids per-lookup `String` clones for the bookkeeping side.
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
        // std HashMap doesn't expose raw_entry stably, so the lookup
        // still needs an owned key — but the used_this_frame side
        // gets a u64 instead of a second clone.
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
        use std::hash::BuildHasher;
        self.hasher.hash_one(key)
    }
}
