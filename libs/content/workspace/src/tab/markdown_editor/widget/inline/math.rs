use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn text_format_math(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code(parent)
    }

    pub fn span_math(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.span_code(node, wrap, range)
    }

    pub fn show_math(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        self.show_code(ui, node, top_left, wrap, range)
    }
}
