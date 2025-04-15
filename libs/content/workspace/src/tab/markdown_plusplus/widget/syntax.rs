use comrak::nodes::AstNode;
use egui::TextFormat;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_syntax(&self, node: &'ast AstNode<'ast>) -> TextFormat {
        let mut text_format = self.text_format(node);
        text_format.color = self.theme.fg().neutral_quarternary;
        text_format
    }
}
