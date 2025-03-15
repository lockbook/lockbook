use comrak::nodes::AstNode;
use egui::TextFormat;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_html_inline(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code(parent)
    }
}
