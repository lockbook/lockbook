use comrak::nodes::{AstNode, NodeMath};
use egui::{Context, FontFamily, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
    MarkdownPlusPlus,
};

pub struct Math<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeMath,
}

impl MarkdownPlusPlus {
    pub fn text_format_math(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code(parent)
    }
}

impl<'a, 't, 'w> Math<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeMath) -> Self {
        Self { ast, node }
    }

    pub fn text_format(
        theme: &Theme, parent_text_format: TextFormat, _ctx: &Context,
    ) -> TextFormat {
        TextFormat {
            color: theme.fg().accent_primary,
            font_id: FontId {
                size: parent_text_format.font_id.size * 0.9,
                family: FontFamily::Monospace,
            },
            background: theme.bg().neutral_secondary,
            ..parent_text_format
        }
    }
}

impl Inline for Math<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui) {
        self.ast
            .show_text(wrap, top_left, ui, self.node.literal.clone());
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.text_span(wrap, ctx, self.node.literal.clone())
    }
}
