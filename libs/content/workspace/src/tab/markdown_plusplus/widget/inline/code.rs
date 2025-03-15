use comrak::nodes::AstNode;
use egui::TextFormat;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_code(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat {
            color: self.theme.fg().accent_primary,
            background: self.theme.bg().neutral_secondary,
            ..self.text_format_code_block(parent)
        }
    }
}
