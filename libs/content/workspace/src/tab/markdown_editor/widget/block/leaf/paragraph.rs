use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt, RangeIterExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::block::APPROX_CHAR_WIDTH_FACTOR;

impl<'ast> MdRender {
    pub fn height_paragraph(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut result = 0.;
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            result += self.height_paragraph_line(node, node_line);

            if line_idx != last_line_idx {
                result += self.layout.block_spacing;
            }
        }

        result
    }

    /// Total image height + block_spacing for any image whose source
    /// position lies within `node_line`. Painted above the line text.
    fn paragraph_line_image_height(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme),
    ) -> f32 {
        let mut h = 0.0;
        for descendant in node.descendants() {
            if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                if node_line.contains_inclusive(self.node_range(descendant).start()) {
                    let NodeLink { url, .. } = &**node_link;
                    h += self.height_image(node, url);
                    h += self.layout.block_spacing;
                }
            }
        }
        h
    }

    /// Cheap per-source-line approximation matching the structure of
    /// `height_approx`'s paragraph branch — uses buffer character
    /// count and `APPROX_CHAR_WIDTH_FACTOR`. Used by the scroll area
    /// for per-line row sizing.
    pub fn height_approx_paragraph_line(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme),
    ) -> f32 {
        let row_height = self.row_height(node);
        let width = self.width(node).max(row_height);
        let chars = self.buffer[node_line].chars().count();
        let char_width = row_height * APPROX_CHAR_WIDTH_FACTOR.with(|c| c.get());
        let chars_per_row = (width / char_width).floor().max(1.0) as usize;
        let rows = ((chars as f32) / chars_per_row as f32).ceil().max(1.0);
        rows * row_height + (rows - 1.0).max(0.0) * self.layout.row_spacing
    }

    pub fn height_paragraph_line(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme),
    ) -> f32 {
        let width = self.width(node);
        let mut wrap = self.new_wrap(width);

        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        if let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        {
            if !pre_node.is_empty() {
                wrap.offset += self.span_section(&wrap, pre_node, self.text_format(node));
            }
            if !pre_children.is_empty() {
                wrap.offset += self.span_section(&wrap, pre_children, self.text_format(node));
            }
            wrap.offset += self.inline_children_span(node, &wrap, node_line);
            if !post_children.is_empty() {
                wrap.offset += self.span_section(&wrap, post_children, self.text_format(node));
            }
            if !post_node.is_empty() {
                wrap.offset += self.span_section(&wrap, post_node, self.text_format(node));
            }
        } else {
            // This handles empty paragraph lines such as in "- [ ] \n  x"
            wrap.offset += self.span_section(&wrap, node_line, self.text_format(node));
        };

        self.paragraph_line_image_height(node, node_line) + wrap.height()
    }

    pub fn show_paragraph(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let node_line = self.node_line(node, line);

            self.show_paragraph_line(ui, node, top_left, node_line);
            top_left.y += self.height_paragraph_line(node, node_line);
            top_left.y += self.layout.block_spacing;
        }
    }

    pub fn show_paragraph_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        node_line: (Grapheme, Grapheme),
    ) {
        // Inline images: paint each one above the line text and
        // advance the local top_left so text starts beneath them.
        for descendant in node.descendants() {
            if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                if node_line.contains_inclusive(self.node_range(descendant).start()) {
                    let NodeLink { url, .. } = &**node_link;
                    self.show_image_block(ui, node, top_left, url);
                    top_left.y += self.height_image(node, url);
                    top_left.y += self.layout.block_spacing;
                }
            }
        }

        let width = self.width(node);
        let mut wrap = self.new_wrap(width);

        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        if let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        {
            if !pre_node.is_empty() {
                self.show_section(ui, top_left, &mut wrap, pre_node, self.text_format(node));
            }
            if !pre_children.is_empty() {
                self.show_section(ui, top_left, &mut wrap, pre_children, self.text_format(node));
            }
            self.show_inline_children(ui, node, top_left, &mut wrap, node_line);
            if !post_children.is_empty() {
                self.show_section(ui, top_left, &mut wrap, post_children, self.text_format(node));
            }
            if !post_node.is_empty() {
                self.show_section(ui, top_left, &mut wrap, post_node, self.text_format(node));
            }
        };

        self.bounds.wrap_lines.extend(wrap.row_ranges);
    }

    pub fn compute_bounds_paragraph(&mut self, node: &'ast AstNode<'ast>) {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let node_line = self.node_line(node, line);

            self.bounds.inline_paragraphs.push(node_line);
        }
    }
}
