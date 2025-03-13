use egui::{Context, Pos2, Ui};

use crate::tab::markdown_plusplus::widget::{Ast, Inline, WrapContext};

pub struct SoftBreak<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
}

impl<'a, 't, 'w> SoftBreak<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { ast }
    }
}

impl Inline for SoftBreak<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui) {
        wrap.offset = wrap.line_end();
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        wrap.line_remaining()
    }
}
