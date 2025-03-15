use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_html_block(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code_block(parent)
    }

    pub fn height_html_block(&self, node: &AstNode<'_>, width: f32, html: &str) -> f32 {
        self.height_code_block(node, width, html, "html")
    }

    pub fn show_html_block(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32, html: &str,
    ) {
        self.show_code_block(ui, node, top_left, width, html, "html");
    }
}
