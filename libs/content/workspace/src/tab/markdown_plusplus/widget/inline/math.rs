use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_math(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code(parent)
    }

    pub fn span_math(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext, literal: &str) -> f32 {
        self.span_node_text_line(node, wrap, literal)
    }

    pub fn show_math(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        let sourcepos = node.data.borrow().sourcepos;
        let range = self.sourcepos_to_range(sourcepos);

        self.show_node_text_line(ui, node, top_left, wrap, range)
    }
}
