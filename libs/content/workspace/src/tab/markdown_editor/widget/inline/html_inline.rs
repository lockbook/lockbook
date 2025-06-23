use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_html_inline(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code(parent)
    }

    pub fn span_html_inline(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        self.span_text_line(wrap, self.node_range(node), self.text_format(node))
    }

    pub fn show_html_inline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        self.show_text_line(
            ui,
            top_left,
            wrap,
            self.node_range(node),
            self.text_format(node),
            false,
        );
    }
}
