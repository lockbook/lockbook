use comrak::nodes::NodeValue;
use egui::{Context, Pos2, Ui, Vec2};

use crate::tab::markdown_plusplus::widget::{
    Ast, Block, WrapContext, BLOCK_SPACING, TABLE_PADDING,
};

pub struct TableCell<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
}

impl<'a, 't, 'w> TableCell<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { ast }
    }
}

impl Block for TableCell<'_, '_, '_> {
    fn show(&self, mut width: f32, mut top_left: Pos2, ui: &mut Ui) {
        top_left += Vec2::splat(TABLE_PADDING);
        width -= 2.0 * TABLE_PADDING;

        for descendant in self.ast.node.descendants() {
            if matches!(descendant.data.borrow().value, NodeValue::Image(_)) {
                let descendent =
                    Ast::new(descendant, self.ast.text_format.clone(), self.ast.theme, ui.ctx());
                Block::show(&descendent, width, top_left, ui);
                top_left.y += Block::height(&descendent, width, ui.ctx());
                top_left.y += BLOCK_SPACING;
            }
        }

        self.ast
            .show_inline_children(&mut WrapContext::new(width), &mut top_left, ui);
    }

    fn height(&self, mut width: f32, ctx: &Context) -> f32 {
        width -= 2.0 * TABLE_PADDING;

        let mut images_height = 0.;
        for descendant in self.ast.node.descendants() {
            if matches!(descendant.data.borrow().value, NodeValue::Image(_)) {
                images_height += Block::height(
                    &Ast::new(descendant, self.ast.text_format.clone(), self.ast.theme, ctx),
                    width,
                    ctx,
                );
                images_height += BLOCK_SPACING;
            }
        }

        images_height
            + self
                .ast
                .inline_children_height(width - 2. * TABLE_PADDING, ctx)
            + 2.0 * TABLE_PADDING
    }
}
