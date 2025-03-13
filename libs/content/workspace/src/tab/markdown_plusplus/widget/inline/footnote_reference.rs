use comrak::nodes::{AstNode, NodeFootnoteReference};
use egui::{Context, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
    MarkdownPlusPlus,
};

pub struct FootnoteReference<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeFootnoteReference,
}

impl MarkdownPlusPlus {
    pub fn text_format_footnote_reference(&self, parent: &AstNode<'_>) -> TextFormat {
        let theme = self.theme();
        TextFormat { color: theme.fg().neutral_tertiary, ..self.text_format_superscript(parent) }
    }
}

impl<'a, 't, 'w> FootnoteReference<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeFootnoteReference) -> Self {
        Self { ast, node }
    }

    pub fn text_format(
        theme: &Theme, parent_text_format: TextFormat, _ctx: &Context,
    ) -> TextFormat {
        TextFormat {
            color: theme.fg().neutral_tertiary,
            font_id: FontId { size: 10., ..parent_text_format.font_id },
            ..parent_text_format
        }
    }
}

impl Inline for FootnoteReference<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui) {
        self.ast
            .show_text(wrap, top_left, ui, format!("{}", self.node.ix));
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.text_span(wrap, ctx, format!("{}", self.node.ix))
    }
}
