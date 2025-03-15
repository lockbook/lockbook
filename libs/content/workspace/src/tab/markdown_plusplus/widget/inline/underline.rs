use comrak::nodes::AstNode;
use egui::{Stroke, TextFormat};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_underline(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            underline: Stroke { width: 1., color: parent_text_format.color },
            ..parent_text_format
        }
    }
}
