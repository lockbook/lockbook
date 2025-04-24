use comrak::nodes::AstNode;
use egui::TextFormat;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_syntax(&self, node: &'ast AstNode<'ast>) -> TextFormat {
        let mono = self.text_format_code(node);
        TextFormat {
            color: self.theme.fg().neutral_quarternary,
            background: Default::default(),
            underline: Default::default(),
            strikethrough: Default::default(),
            italics: Default::default(),
            ..mono
        }
    }
}
