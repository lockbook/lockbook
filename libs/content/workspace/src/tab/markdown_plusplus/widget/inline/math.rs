use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::widget::utils::text_layout::Wrap;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_math(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code(parent)
    }

    pub fn span_math(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        self.span_code(node, wrap)
    }

    pub fn show_math(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        self.show_code(ui, node, top_left, wrap);
    }
}
