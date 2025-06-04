use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::widget::utils::text_layout::Wrap;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_strong(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId {
                family: FontFamily::Name(Arc::from("Bold")),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn span_strong(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_strong(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        self.show_circumfix(ui, node, top_left, wrap);
    }
}
