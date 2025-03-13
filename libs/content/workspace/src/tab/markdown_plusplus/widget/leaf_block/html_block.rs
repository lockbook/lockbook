use comrak::nodes::{AstNode, NodeHtmlBlock};
use egui::{Context, FontFamily, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Block},
    MarkdownPlusPlus,
};

pub struct HtmlBlock<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeHtmlBlock,
}

impl MarkdownPlusPlus {
    pub fn text_format_html_block(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code_block(parent)
    }
}

impl<'a, 't, 'w> HtmlBlock<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeHtmlBlock) -> Self {
        Self { ast, node }
    }

    pub fn text_format(
        _theme: &Theme, parent_text_format: TextFormat, _ctx: &Context,
    ) -> TextFormat {
        TextFormat {
            font_id: FontId {
                size: parent_text_format.font_id.size * 0.9,
                family: FontFamily::Monospace,
            },
            ..parent_text_format
        }
    }
}

impl Block for HtmlBlock<'_, '_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        self.ast
            .show_code_block(width, top_left, ui, self.node.literal.clone(), "html".into());
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast
            .code_block_height(width, ctx, self.node.literal.clone(), "html".into())
    }
}
