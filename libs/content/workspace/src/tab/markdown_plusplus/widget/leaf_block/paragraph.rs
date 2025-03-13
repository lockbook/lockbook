use egui::{Context, Pos2, Ui};

use crate::tab::markdown_plusplus::widget::{Ast, Block, WrapContext};

pub struct Paragraph<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
}

impl<'a, 't, 'w> Paragraph<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { ast }
    }
}

impl Block for Paragraph<'_, '_, '_> {
    fn show(&self, width: f32, mut top_left: Pos2, ui: &mut Ui) {
        self.ast
            .show_inline_children(&mut WrapContext::new(width), &mut top_left, ui);
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast.inline_children_height(width, ctx)
    }
}
