use comrak::nodes::AstNode;
use egui::{FontId, TextFormat};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_superscript(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId { size: 10., ..parent_text_format.font_id },
            ..parent_text_format
        }
    }
}
