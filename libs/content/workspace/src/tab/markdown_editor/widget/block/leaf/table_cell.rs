use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui, Vec2};
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
            if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                let NodeLink { url, .. } = &**node_link;
                images_height += self.height_image(node, url);
                images_height += BLOCK_SPACING;
            }
        }

        wrap.offset += self.inline_children_span(node, &wrap, node_line);

        BLOCK_PADDING + images_height + wrap.height() + BLOCK_PADDING
    }

    pub fn width_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let row = node.parent().unwrap();
        self.width(row) / row.children().count() as f32
    }

    pub fn show_table_cell(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        top_left += Vec2::splat(BLOCK_PADDING);
        let width = self.width(node) - 2.0 * BLOCK_PADDING;
        let mut wrap = Wrap::new(width);
        let node_line = self.node_range(node); // table cells are always single-line

        for descendant in node.descendants() {
            if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                let NodeLink { url, .. } = &**node_link;
                self.show_image_block(ui, node, top_left, url);
                top_left.y += self.height_image(node, url);
                top_left.y += BLOCK_SPACING;
            }
        }

        self.show_inline_children(ui, node, top_left, &mut wrap, node_line);

        self.bounds.wrap_lines.extend(wrap.row_ranges);
    }

    pub fn compute_bounds_table_cell(&mut self, node: &'ast AstNode<'ast>) {
        let node_line = self.node_range(node); // table cells are always single-line
        self.bounds.inline_paragraphs.push(node_line);
    }
}
