use egui::{Context, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
};

pub struct EscapedTag<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    escaped: &'w String,
}

impl<'a, 't, 'w> EscapedTag<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, escaped: &'w String) -> Self {
        Self { ast, escaped }
    }

    // rendered as code
    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        parent_text_format
    }
}

impl Inline for EscapedTag<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui) {
        self.ast.show_text(wrap, top_left, ui, self.escaped.clone());
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.text_span(wrap, ctx, self.escaped.clone())
    }
}
