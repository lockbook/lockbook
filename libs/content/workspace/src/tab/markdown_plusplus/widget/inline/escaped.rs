use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_escaped(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format(parent)
    }

    pub fn span_escaped(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_escaped(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        self.show_circumfix(ui, node, top_left, wrap);
    }
}
