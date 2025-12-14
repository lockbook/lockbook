use comrak::nodes::AstNode;
use egui::{Color32, Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn background_color_highlight(&self) -> Color32 {
        self.theme.bg().yellow.gamma_multiply(0.35)
    }

    pub fn text_format_highlight(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat { background: self.background_color_highlight(), ..self.text_format(parent) }
    }

    pub fn span_highlight(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.circumfix_span(node, wrap, range)
    }

    pub fn show_highlight(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        self.show_circumfix(ui, node, top_left, wrap, range)
    }
}
