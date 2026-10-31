use crate::tab::markdown_editor::{syntax_set, syntax_theme};
use comrak::nodes::AstNode;
use egui::{Color32, Pos2, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
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
            let last = self.bounds.source_lines.len() - 1;
            let mut result = 0.;
            for line_idx in 0..self.bounds.source_lines.len() {
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
            let last = self.bounds.source_lines.len() - 1;
            for line_idx in 0..self.bounds.source_lines.len() {
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
        if self.mask {
            // One `*` per source byte keeps the `one_to_one` (byte-indexed)
            // mapping exact — cursor, selection, and iOS text rects line up
            // while the real text never reaches the screen.
            if !line.is_empty() {
                let masked = "*".repeat(self.buffer[line].len());
                layout.push_source(line, &masked, self.text_format_syntax());
            }
            return layout;
        }
        let clickable = self.readonly || self.ctx.input(|i| i.modifiers.command);
        let sense = self
            .link_sense(clickable)
            .filter(|_| !self.bounds.source_links.is_empty());
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
                self.push_source_run(&mut layout, line, region, text_format.clone(), sense);
            }
        } else {
            self.push_source_run(&mut layout, line, line, self.text_format_syntax(), sense);
        }
        layout
    }

    /// Pushes `run` of source `line`, split where links on the line start
    /// and end so each link's text sits in its click area.
    fn push_source_run(
        &self, layout: &mut Layout, line: (Grapheme, Grapheme), run: (Grapheme, Grapheme),
        format: Format, sense: Option<egui::Sense>,
    ) {
        if run.0 == run.1 {
            return;
        }
        // links as clipped to the line, outermost first where they start together
        let mut links: Vec<_> = self
            .bounds
            .source_links
            .iter()
            .map(|&link| (link, (link.0.max(line.0), link.1.min(line.1))))
            .filter(|(_, on_line)| on_line.0 < on_line.1 && sense.is_some())
            .collect();
        links.sort_by_key(|(_, on_line)| (on_line.0, std::cmp::Reverse(on_line.1)));
        let mut cuts: Vec<Grapheme> = links
            .iter()
            .flat_map(|(_, on_line)| [on_line.0, on_line.1])
            .filter(|&at| run.0 < at && at < run.1)
            .collect();
        cuts.sort_unstable();
        cuts.dedup();

        let mut start = run.0;
        for end in cuts.into_iter().chain([run.1]) {
            for (link, on_line) in &links {
                if on_line.0 == start {
                    let salt = Self::link_interaction_id_salt(*link);
                    layout.interaction_open(salt, sense.unwrap());
                }
            }
            if start < end {
                layout.push_source((start, end), &self.buffer[(start, end)], format.clone());
            }
            for _ in links.iter().filter(|(_, on_line)| on_line.1 == end) {
                layout.interaction_close();
            }
            start = end;
        }
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
        if any_children && !self.plaintext {
            self.compute_bounds_block_children(node);
        } else {
            for line_idx in 0..self.bounds.source_lines.len() {
                let line = self.bounds.source_lines[line_idx];
                self.bounds.inline_paragraphs.push(line);
            }
        }
    }
}
