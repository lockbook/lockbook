use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};

impl<'ast> MdRender {
    pub fn text_format_escaped(&self, parent: &AstNode<'_>) -> Format {
        self.text_format(parent)
    }

    pub fn span_escaped(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (Grapheme, Grapheme),
    ) -> f32 {
        self.circumfix_span(node, wrap, range)
    }

    pub fn show_escaped(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (Grapheme, Grapheme),
    ) -> Response {
        self.show_circumfix(ui, node, top_left, wrap, range)
    }
}
