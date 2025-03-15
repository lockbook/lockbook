use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_multiline_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_block_quote(parent)
    }

    pub fn height_multiline_block_quote(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        self.height_block_quote(node, width)
    }

    pub fn show_multiline_block_quote(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32,
    ) {
        self.show_block_quote(ui, node, top_left, width);
    }
}
