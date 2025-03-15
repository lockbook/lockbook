use comrak::nodes::AstNode;
use egui::TextFormat;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_emph(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { italics: true, ..parent_text_format }
    }
}
