use comrak::nodes::AstNode;
use egui::TextFormat;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_wiki_link(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_link(parent)
    }
}
