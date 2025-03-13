use egui::{Context, Pos2, Ui};

use crate::tab::markdown_plusplus::widget::{Ast, Inline, WrapContext};

pub struct SoftBreak<'a, 't, 'w> {
    _ast: &'w Ast<'a, 't>,
}

impl<'a, 't, 'w> SoftBreak<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { _ast: ast }
    }
}

impl Inline for SoftBreak<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, _top_left: Pos2, _ui: &mut Ui) {
        wrap.offset = wrap.line_end();
    }

    fn span(&self, wrap: &WrapContext, _ctx: &Context) -> f32 {
        wrap.line_remaining()
    }
}
