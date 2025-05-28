use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt, RangeExt, RangeIterExt as _};

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

        let last_line_idx = self.node_lines(node).iter().count() - 1;
        for (line_idx, line) in self.node_lines(node).iter().enumerate() {
            let line = self.bounds.source_lines[line];

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

        let line_children = self.children_in_line(node, line);

        if let Some(first_child) = line_children.first() {
            if node_line.intersects(&self.buffer.current.selection, true) {
                let prefix_range = (node_line.start(), self.node_range(first_child).start());
                wrap.offset +=
                    self.span_text_line(&wrap, prefix_range, self.text_format_syntax(node));
            }
        }
        for child in &line_children {
            wrap.offset += self.span(child, &wrap);
        }
        if let Some(last_child) = line_children.last() {
            if node_line.intersects(&self.buffer.current.selection, true) {
                let postfix_range = (self.node_range(last_child).end(), node_line.end());
                wrap.offset +=
                    self.span_text_line(&wrap, postfix_range, self.text_format_syntax(node));
            }
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
        let node_line = self.node_line(node, line);

        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        if let Some((leading_whitespace, _, children, postfix_whitespace, _)) =
            self.line_ranges(node, line)
        {
            if !leading_whitespace.is_empty() {
                self.bounds.paragraphs.push(leading_whitespace);
            }
            self.bounds.paragraphs.push(children);
            if !postfix_whitespace.is_empty() {
                self.bounds.paragraphs.push(postfix_whitespace);
            }

            let mut wrap = Wrap::new(self.width(node));
            let reveal = node_line.intersects(&self.buffer.current.selection, true);
            if reveal {
                self.show_text_line(
                    ui,
                    top_left,
                    &mut wrap,
                    leading_whitespace,
                    self.text_format_syntax(node),
                    false,
                );
            }
            for child in &self.children_in_line(node, line) {
                self.show_inline(ui, child, top_left, &mut wrap);
            }
            if reveal {
                self.show_text_line(
                    ui,
                    top_left,
                    &mut wrap,
                    postfix_whitespace,
                    self.text_format_syntax(node),
                    false,
                );
            }
        } else {
            // todo: probably wrong - don't we want to show as syntax, or at least map the whole range?
            self.bounds.paragraphs.push(node_line.start().into_range());
        }
    }
}
