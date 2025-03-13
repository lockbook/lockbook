use comrak::nodes::{AstNode, NodeMultilineBlockQuote};
use egui::{Context, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Block},
    MarkdownPlusPlus,
};

pub struct MultilineBlockQuote<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    _node: &'w NodeMultilineBlockQuote,
}

impl MarkdownPlusPlus {
    pub fn text_format_multiline_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_block_quote(parent)
    }
}

impl<'a, 't, 'w> MultilineBlockQuote<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeMultilineBlockQuote) -> Self {
        Self { ast, _node: node }
    }

    pub fn text_format(
        theme: &Theme, parent_text_format: TextFormat, _ctx: &Context,
    ) -> TextFormat {
        TextFormat { color: theme.fg().neutral_tertiary, ..parent_text_format }
    }
}

impl Block for MultilineBlockQuote<'_, '_, '_> {
    fn show(&self, width: f32, mut top_left: Pos2, ui: &mut Ui) {
        top_left.x += ui.style().spacing.indent;
        self.ast.show_block_children(width, top_left, ui)
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast.block_children_height(width, ctx)
    }
}
