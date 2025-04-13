use comrak::nodes::AstNode;
use egui::{FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_superscript(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId { size: 10., ..parent_text_format.font_id },
            ..parent_text_format
        }
    }

    pub fn span_superscript(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_superscript(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        self.show_circumfix(ui, node, top_left, wrap);
    }
}
