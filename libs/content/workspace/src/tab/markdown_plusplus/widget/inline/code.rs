use comrak::nodes::{AstNode, NodeCode};
use egui::{Context, FontFamily, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
    MarkdownPlusPlus,
};

pub struct Code<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeCode,
}

impl MarkdownPlusPlus {
    pub fn text_format_code(&self, parent: &AstNode<'_>) -> TextFormat {
        let theme = self.theme();
        TextFormat {
            color: theme.fg().accent_primary,
            background: theme.bg().neutral_secondary,
            ..self.text_format_code_block(parent)
        }
    }
}

impl<'a, 't, 'w> Code<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeCode) -> Self {
        Self { ast, node }
    }

    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        let parent_row_height = ctx.fonts(|fonts| fonts.row_height(&parent_text_format.font_id));
        let monospace_row_height = ctx.fonts(|fonts| {
            fonts
                .row_height(&FontId { family: FontFamily::Monospace, ..parent_text_format.font_id })
        });
        let monospace_row_height_preserving_size =
            parent_text_format.font_id.size * parent_row_height / monospace_row_height;
        TextFormat {
            color: theme.fg().accent_primary,
            font_id: FontId {
                size: monospace_row_height_preserving_size,
                family: FontFamily::Monospace,
            },
            background: theme.bg().neutral_secondary,
            line_height: Some(parent_row_height),
            ..parent_text_format
        }
    }
}

impl Inline for Code<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui) {
        self.ast
            .show_text(wrap, top_left, ui, self.node.literal.clone());
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.text_span(wrap, ctx, self.node.literal.clone())
    }
}
