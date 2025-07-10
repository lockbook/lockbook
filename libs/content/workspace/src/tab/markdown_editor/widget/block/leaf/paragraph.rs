use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt, RangeIterExt as _};

use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::widget::BLOCK_SPACING;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn height_paragraph(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut result = 0.;
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                result += self.height_image(node, url);
                result += BLOCK_SPACING;
            }
        }

        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];

            result += self.height_paragraph_line(node, line);

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
        let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        else {
            unreachable!("Paragraphs always have children")
        };

        let reveal = node_line.intersects(&self.buffer.current.selection, true);
        if reveal {
            wrap.offset += self.span_text_line(&wrap, pre_node, self.text_format_syntax(node));
            wrap.offset += self.span_text_line(&wrap, pre_children, self.text_format_syntax(node));
        }
        wrap.offset += self.inline_children_span(node, &wrap, node_line);
        if reveal {
            wrap.offset += self.span_text_line(&wrap, post_children, self.text_format_syntax(node));
            wrap.offset += self.span_text_line(&wrap, post_node, self.text_format_syntax(node));
        }

        wrap.height()
    }

    pub fn show_paragraph(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                self.show_image_block(ui, node, top_left, url);
                top_left.y += self.height_image(node, url);
                top_left.y += BLOCK_SPACING;
            }
        }

        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let node_line = self.node_line(node, line);

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
        let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        else {
            unreachable!("Paragraphs always have children")
        };

        if !pre_node.is_empty() {
            self.show_text_line(ui, top_left, &mut wrap, pre_node, self.text_format(node), false);
        }
        if !pre_children.is_empty() {
            self.show_text_line(
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
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                post_children,
                self.text_format(node),
                false,
            );
        }
        if !post_node.is_empty() {
            self.show_text_line(ui, top_left, &mut wrap, post_node, self.text_format(node), false);
        }

        self.bounds.wrap_lines.extend(wrap.row_ranges);
    }

    pub fn compute_bounds_paragraph(&mut self, node: &'ast AstNode<'ast>) {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let node_line = self.node_line(node, line);

            self.compute_bounds_paragraph_line(node, node_line);
        }
    }

    pub fn compute_bounds_paragraph_line(
        &mut self, node: &'ast AstNode<'ast>, node_line: (DocCharOffset, DocCharOffset),
    ) {
        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        let Some((pre_node, pre_children, children, post_children, post_node)) =
            self.split_range(node, node_line)
        else {
            unreachable!("Paragraphs always have children")
        };

        if !pre_node.is_empty() {
            self.bounds.paragraphs.push(pre_node);
        }
        if !pre_children.is_empty() {
            self.bounds.paragraphs.push(pre_children);
        }
        self.bounds.paragraphs.push(children);
        self.bounds.inline_paragraphs.push(children);
        if !post_children.is_empty() {
            self.bounds.paragraphs.push(post_children);
        }
        if !post_node.is_empty() {
            self.bounds.paragraphs.push(post_node);
        }
    }
}
