use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};

use crate::tab::markdown_plusplus::{
    widget::{WrapContext, BLOCK_SPACING, TABLE_PADDING},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn height_paragraph(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        let mut images_height = 0.;
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                images_height += self.height_image(width, url);
                images_height += BLOCK_SPACING;
            }
        }

        images_height + self.inline_children_height(node, width - 2. * TABLE_PADDING)
    }

    pub fn show_paragraph(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, wrap: &mut WrapContext,
    ) {
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                self.show_image_block(ui, top_left, wrap.width, url);
                top_left.y += self.height_image(wrap.width, url);
                top_left.y += BLOCK_SPACING;
            }
        }

        self.show_inline_children(ui, node, top_left, wrap);
    }
}
