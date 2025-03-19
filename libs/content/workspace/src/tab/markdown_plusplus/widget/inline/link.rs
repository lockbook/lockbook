use comrak::nodes::AstNode;
use egui::{Stroke, TextFormat};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_link(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: self.theme.fg().blue,
            underline: Stroke { width: 1., color: self.theme.fg().blue },
            ..parent_text_format
        }
    }
}
