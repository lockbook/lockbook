use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_escaped(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format(parent)
    }

    pub fn span_escaped(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_escaped(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) -> Response {
        self.show_circumfix(ui, node, top_left, wrap)
    }
}
