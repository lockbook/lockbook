use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, BLOCK_PADDING, BLOCK_SPACING},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn height_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut width = self.width(node);
        width -= 2.0 * BLOCK_PADDING;

        let mut images_height = 0.;
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                images_height += self.height_image(node, url);
                images_height += BLOCK_SPACING;
            }
        }

        images_height
            + self.inline_children_height(node, width - 2. * BLOCK_PADDING)
            + 2.0 * BLOCK_PADDING
    }

    pub fn show_table_cell(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let width = self.width(node);
        let wrap = Wrap::new(width);

        top_left += Vec2::splat(BLOCK_PADDING);
        let width = wrap.width - 2.0 * BLOCK_PADDING;

        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                self.show_image_block(ui, node, top_left, url);
                top_left.y += self.height_image(node, url);
                top_left.y += BLOCK_SPACING;
            }
        }

        self.show_inline_children(ui, node, top_left, &mut Wrap::new(width));

        // bounds
        let sourcepos = node.data.borrow().sourcepos;
        let range = self.sourcepos_to_range(sourcepos);
        self.bounds.paragraphs.push(range);
    }
}
