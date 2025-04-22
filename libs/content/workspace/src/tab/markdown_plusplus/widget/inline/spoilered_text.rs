use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::Wrap, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_spoilered_text(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { background: self.theme.bg().neutral_tertiary, ..parent_text_format }
    }

    pub fn span_spoilered_text(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_spoilered_text(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        self.show_circumfix(ui, node, top_left, wrap);
    }
}
