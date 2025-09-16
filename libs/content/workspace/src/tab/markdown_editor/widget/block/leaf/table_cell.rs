use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::tab::markdown_editor::widget::{BLOCK_PADDING, BLOCK_SPACING};

impl<'ast> Editor {
    pub fn height_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node) - 2.0 * BLOCK_PADDING;
        let mut wrap = Wrap::new(width);
        let node_line = self.node_range(node); // table cells are always single-line

        let mut images_height = 0.;
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                images_height += self.height_image(node, url);
                images_height += BLOCK_SPACING;
            }
        }

        if let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        {
            let reveal = node_line.intersects(&self.buffer.current.selection, true);
            if reveal {
                wrap.offset += self.span_section(&wrap, pre_node, self.text_format_syntax(node));
                wrap.offset +=
                    self.span_section(&wrap, pre_children, self.text_format_syntax(node));
            }
            wrap.offset += self.inline_children_span(node, &wrap, node_line);
            if reveal {
                wrap.offset +=
                    self.span_section(&wrap, post_children, self.text_format_syntax(node));
                wrap.offset += self.span_section(&wrap, post_node, self.text_format_syntax(node));
            }
        } else {
            wrap.offset += self.span_section(&wrap, node_line, self.text_format_syntax(node));
        }

        images_height + wrap.height()
    }

    pub fn width_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let row = node.parent().unwrap();
        self.width(row) / row.children().count() as f32
    }

    pub fn show_table_cell(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        top_left.x += BLOCK_PADDING;
        let width = self.width(node) - 2.0 * BLOCK_PADDING;
        let mut wrap = Wrap::new(width);
        let node_line = self.node_range(node); // table cells are always single-line

        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                self.show_image_block(ui, node, top_left, url);
                top_left.y += self.height_image(node, url);
                top_left.y += BLOCK_SPACING;
            }
        }

        let reveal = node_line.intersects(&self.buffer.current.selection, true);
        if let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        {
            if reveal {
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    pre_node,
                    self.text_format_syntax(node),
                    false,
                );
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    pre_children,
                    self.text_format_syntax(node),
                    false,
                );
            }
            self.show_inline_children(ui, node, top_left, &mut wrap, node_line);
            if reveal {
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    post_children,
                    self.text_format_syntax(node),
                    false,
                );
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    post_node,
                    self.text_format_syntax(node),
                    false,
                );
            }
        } else {
            self.show_section(ui, top_left, &mut wrap, node_line, self.text_format(node), false);
        }

        self.bounds.wrap_lines.extend(wrap.row_ranges);
    }

    pub fn compute_bounds_table_cell(&mut self, node: &'ast AstNode<'ast>) {
        let node_line = self.node_range(node); // table cells are always single-line
        self.bounds.inline_paragraphs.push(node_line);

        if let Some((pre_node, pre_children, children, post_children, post_node)) =
            self.split_range(node, node_line)
        {
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
        } else {
            #[allow(clippy::collapsible_else_if)]
            if !node_line.is_empty() {
                self.bounds.paragraphs.push(node_line);
                self.bounds.inline_paragraphs.push(node_line);
            }
        }
    }
}
