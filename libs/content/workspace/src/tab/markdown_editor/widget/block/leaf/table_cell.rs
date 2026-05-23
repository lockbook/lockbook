use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui, Vec2};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

impl<'ast> MdRender {
    fn layout_table_cell(&self, node: &'ast AstNode<'ast>) -> Layout {
        let node_line = self.node_range(node); // table cells are always single-line
        let mut layout = Layout::new(node_line);
        self.layout_inline_children(&mut layout, node, node_line);
        layout
    }

    pub fn height_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node) - 2.0 * self.layout.block_padding;
        let mut images_height = 0.;
        for descendant in node.descendants() {
            if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                let NodeLink { url, .. } = &**node_link;
                images_height += self.height_image(node, url);
                images_height += self.layout.block_spacing;
            }
        }
        let content =
            self.compute_layout_from(self.layout_table_cell(node), width, self.layout.row_height);
        self.layout.block_padding + images_height + content.height + self.layout.block_padding
    }

    pub fn width_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let row = node.parent().unwrap();
        self.width(row) / row.children().count() as f32
    }

    pub fn show_table_cell(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        top_left += Vec2::splat(self.layout.block_padding);
        let width = self.width(node) - 2.0 * self.layout.block_padding;

        for descendant in node.descendants() {
            if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                let NodeLink { url, .. } = &**node_link;
                self.show_image_block(ui, node, top_left, url);
                top_left.y += self.height_image(node, url);
                top_left.y += self.layout.block_spacing;
            }
        }
        let result =
            self.compute_layout_from(self.layout_table_cell(node), width, self.layout.row_height);
        self.show_wrap_layout(ui, top_left, &result);
    }

    pub fn compute_bounds_table_cell(&mut self, node: &'ast AstNode<'ast>) {
        let node_line = self.node_range(node); // table cells are always single-line

        self.bounds.inline_paragraphs.push(node_line);
    }
}
