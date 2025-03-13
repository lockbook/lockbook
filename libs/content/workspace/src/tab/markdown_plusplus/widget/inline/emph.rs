use egui::{Context, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
};

pub struct Emph<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
}

impl<'a, 't, 'w> Emph<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { ast }
    }

    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        TextFormat { italics: true, ..parent_text_format }
    }
}

impl Inline for Emph<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, mut top_left: Pos2, ui: &mut Ui) {
        self.ast.show_inline_children(wrap, &mut top_left, ui)
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.inline_children_span(wrap, ctx)
    }
}
