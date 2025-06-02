use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt, RangeIterExt as _};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, BLOCK_SPACING},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
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
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        let node_line = self.node_line(node, line);

        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.line_ranges(node, node_line)
        else {
            unreachable!("Paragraphs always have children")
        };

        let reveal = node_line.intersects(&self.buffer.current.selection, true);
        if reveal {
            wrap.offset += self.span_text_line(&wrap, pre_node, self.text_format_syntax(node));
            wrap.offset += self.span_text_line(&wrap, pre_children, self.text_format_syntax(node));
        }
        for child in &self.children_in_range(node, node_line) {
            wrap.offset += self.span(child, &wrap);
        }
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

            let line_height = self.height_paragraph_line(node, line);

            self.show_paragraph_line(ui, node, top_left, line);
            top_left.y += line_height;

            top_left.y += BLOCK_SPACING;
        }
    }

    pub fn show_paragraph_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        line: (DocCharOffset, DocCharOffset),
    ) {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        let node_line = self.node_line(node, line);

        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        let Some((pre_node, pre_children, children, post_children, post_node)) =
            self.line_ranges(node, node_line)
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
        if !post_children.is_empty() {
            self.bounds.paragraphs.push(post_children);
        }
        if !post_node.is_empty() {
            self.bounds.paragraphs.push(post_node);
        }

        let reveal = node_line.intersects(&self.buffer.current.selection, true);
        if reveal {
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                pre_node,
                self.text_format_syntax(node),
                false,
            );
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                pre_children,
                self.text_format_syntax(node),
                false,
            );
        }
        for child in &self.children_in_range(node, node_line) {
            self.show_inline(ui, child, top_left, &mut wrap);
        }
        if reveal {
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                post_children,
                self.text_format_syntax(node),
                false,
            );
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                post_node,
                self.text_format_syntax(node),
                false,
            );
        }
    }
}
