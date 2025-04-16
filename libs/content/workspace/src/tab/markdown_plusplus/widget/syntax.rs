use comrak::nodes::AstNode;
use egui::{FontId, TextFormat};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_syntax(&self, node: &'ast AstNode<'ast>) -> TextFormat {
        let parent_text_format =
            self.text_format(node.parent().expect("Documents don't have syntax"));
        TextFormat {
            font_id: FontId {
                family: parent_text_format.font_id.family,
                size: self.text_format(node).font_id.size,
            },
            color: self.theme.fg().neutral_quarternary,
            ..parent_text_format
        }
    }
}
