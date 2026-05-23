use crate::tab::markdown_editor::{syntax_set, syntax_theme};
use comrak::nodes::AstNode;
use egui::{Color32, Pos2, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt as _};
use syntect::easy::HighlightLines;

use crate::show::syntax_ext_for;
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format, Layout};
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn text_format_document(&self) -> Format {
        Format {
            family: FontFamily::Sans,
            bold: false,
            italic: false,
            color: self.ctx.get_lb_theme().neutral_fg(),
            underline: false,
            strikethrough: false,
            background: egui::Color32::TRANSPARENT,
            border: egui::Color32::TRANSPARENT,
            spoiler: false,
            superscript: false,
            subscript: false,
        }
    }

    pub fn height_document(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);

        let any_children = node.children().next().is_some();
        if any_children && !self.plaintext {
            self.block_children_height(node)
        } else {
            let last = self.node_last_line_idx(node);
            let mut result = 0.;
            for line_idx in self.node_lines(node).iter() {
                result += self.height_source_line(line_idx, width);
                if line_idx != last {
                    result += self.layout.row_spacing;
                }
            }
            result
        }
    }

    pub fn show_document(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let width = self.width(node);

        let pre_lines = self.bounds.wrap_lines.len();
        let any_children = node.children().next().is_some();
        if any_children && !self.plaintext {
            self.show_block_children(ui, node, top_left);
        } else {
            let last = self.node_last_line_idx(node);
            for line_idx in self.node_lines(node).iter() {
                let h = self.show_source_line(ui, top_left, line_idx, width);
                top_left.y += h;
                if line_idx != last {
                    top_left.y += self.layout.row_spacing;
                }
            }
        }

        // Empty doc renders nothing — the cursor at offset 0 needs a
        // row anchor to land on. Compute a layout with a zero-visible
        // override at (0, 0) so `bounds.wrap_lines` gets an entry and
        // `fragment_at_pos`/`fragment_at_offset` can resolve.
        if self.bounds.wrap_lines.len() == pre_lines {
            let row_height = self.layout.row_height;
            let mut layout = Layout::new((Grapheme(0), Grapheme(0)));
            layout.push_override((Grapheme(0), Grapheme(0)), "", self.text_format_document());
            let result = self.compute_layout_from(layout, width, row_height);
            self.show_wrap_layout(ui, top_left, &result);
        }
    }

    /// Build a `Layout` for one source line with optional per-region
    /// syntax highlighting (matches the prior plaintext rendering
    /// path). One push_source per highlighted region with that
    /// region's color, or one for the whole line when no highlighter
    /// applies.
    fn layout_source_line(&self, line_idx: usize) -> Layout {
        let line = self.bounds.source_lines[line_idx];
        let mut layout = Layout::new(line);
        let highlighter_syntax = syntax_set().find_syntax_by_extension(syntax_ext_for(&self.ext));
        if let Some(syntax) = highlighter_syntax {
            let mut highlighter = HighlightLines::new(syntax, syntax_theme());
            let line_text = &self.buffer[line];
            let regions = if let Some(regions) = self.syntax.get(line_text, line) {
                regions
            } else {
                let mut regions = Vec::new();
                let mut region_start = self.offset_to_byte(line.start());
                for (style, region_str) in
                    highlighter.highlight_line(line_text, syntax_set()).unwrap()
                {
                    let region_end = region_start + region_str.len();
                    let region = self.range_to_char((region_start, region_end));
                    regions.push((style, region));
                    region_start = region_end;
                }
                self.syntax.insert(line_text.into(), line, regions.clone());
                regions
            };
            let mut text_format = self.text_format_syntax();
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
        } else if !line.is_empty() {
            layout.push_source(line, &self.buffer[line], self.text_format_syntax());
        }
        layout
    }

    /// Renders one source line at `top_left` and returns its height.
    /// Used by both `show_document`'s plaintext branch and by
    /// `DocScrollContent` when iterating per-source-line rows.
    pub fn show_source_line(
        &mut self, ui: &mut Ui, top_left: Pos2, line_idx: usize, width: f32,
    ) -> f32 {
        let layout = self.layout_source_line(line_idx);
        let result = self.compute_layout_from(layout, width, self.layout.row_height);
        let h = result.height;
        self.show_wrap_layout(ui, top_left, &result);
        h
    }

    /// Cheap height estimate for a source line — no shaping. Plaintext
    /// rendering uses a monospace font, so chars-per-row × row_height
    /// approximates the wrapped height without going through cosmic-text.
    /// Suitable for `Rows::approx` in scroll virtualization.
    pub fn height_approx_source_line(&self, line_idx: usize, width: f32) -> f32 {
        let line = self.bounds.source_lines[line_idx];
        let chars = (line.end().0 - line.start().0) as f32;
        let row_height = self.layout.row_height;
        let char_width = row_height * 0.5;
        let chars_per_row = (width / char_width).floor().max(1.0);
        let rows = (chars / chars_per_row).ceil().max(1.0);
        rows * row_height + (rows - 1.0).max(0.0) * self.layout.row_spacing
    }

    /// Cheap measurement of a single source line. Doesn't write to the
    /// syntax cache (callers can be `&self`).
    pub fn height_source_line(&self, line_idx: usize, width: f32) -> f32 {
        let layout = self.layout_source_line(line_idx);
        self.compute_layout_from(layout, width, self.layout.row_height)
            .height
    }

    pub fn compute_bounds_document(&mut self, node: &'ast AstNode<'ast>) {
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                self.bounds.inline_paragraphs.push(line);
            }
        }
    }
}
