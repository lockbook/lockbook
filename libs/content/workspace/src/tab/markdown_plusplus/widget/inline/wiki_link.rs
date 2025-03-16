use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_wiki_link(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_link(parent)
    }

    pub fn span_wiki_link(
        &self, node: &'ast AstNode<'ast>, wrap: &WrapContext, title: &str,
    ) -> f32 {
        self.span_text(node, wrap, title)
    }

    pub fn show_wiki_link(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
        title: &str,
    ) {
        self.show_text(ui, node, top_left, wrap, title);
    }
}
