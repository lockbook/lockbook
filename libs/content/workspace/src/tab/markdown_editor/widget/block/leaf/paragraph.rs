use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt, RangeIterExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::BLOCK_SPACING;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn height_paragraph(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut result = 0.;
        for descendant in node.descendants() {
            if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                let NodeLink { url, .. } = &**node_link;
                result += self.height_image(node, url);
                result += BLOCK_SPACING;
            }
        }

        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            result += self.height_paragraph_line(node, node_line);

            if line_idx != last_line_idx {
                result += BLOCK_SPACING;
            }
        }

        result
    }

    pub fn height_paragraph_line(
        &self, node: &'ast AstNode<'ast>, node_line: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);

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

        wrap.height()
    }

    pub fn show_paragraph(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let node_line = self.node_line(node, line);

            for descendant in node.descendants() {
                if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                    let NodeLink { url, .. } = &**node_link;
                    if node_line.contains_inclusive(self.node_range(descendant).start()) {
                        self.show_image_block(ui, node, top_left, url);
                        top_left.y += self.height_image(node, url);
                        top_left.y += BLOCK_SPACING;
                    }
                }
            }

            let line_height = self.height_paragraph_line(node, node_line);

            self.show_paragraph_line(ui, node, top_left, node_line);
            top_left.y += line_height;

            top_left.y += BLOCK_SPACING;
        }
    }

    pub fn show_paragraph_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_line: (DocCharOffset, DocCharOffset),
    ) {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);

        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        if let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        {
            if !pre_node.is_empty() {
                self.show_section(ui, top_left, &mut wrap, pre_node, self.text_format(node), false);
            }
            if !pre_children.is_empty() {
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    pre_children,
                    self.text_format(node),
                    false,
                );
            }
            self.show_inline_children(ui, node, top_left, &mut wrap, node_line);
            if !post_children.is_empty() {
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    post_children,
                    self.text_format(node),
                    false,
                );
            }
            if !post_node.is_empty() {
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    post_node,
                    self.text_format(node),
                    false,
                );
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
