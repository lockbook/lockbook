use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{Context, FontFamily, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
    MarkdownPlusPlus,
};

pub struct Strong<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
}

impl MarkdownPlusPlus {
    pub fn text_format_strong(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        let theme = self.theme();
        TextFormat {
            color: theme.fg().neutral_primary,
            font_id: FontId {
                family: FontFamily::Name(Arc::from("Bold")),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }
}

impl<'a, 't, 'w> Strong<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { ast }
    }

    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        let _ = ctx;
        TextFormat {
            color: theme.fg().neutral_primary,
            font_id: FontId {
                family: FontFamily::Name(Arc::from("Bold")),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }
}

impl Inline for Strong<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, mut top_left: Pos2, ui: &mut Ui) {
        self.ast.show_inline_children(wrap, &mut top_left, ui)
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.inline_children_span(wrap, ctx)
    }
}
