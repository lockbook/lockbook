use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, TextFormat};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
    pub fn text_format_strong(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: self.theme.fg().neutral_primary,
            font_id: FontId {
                family: FontFamily::Name(Arc::from("Bold")),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }
}
