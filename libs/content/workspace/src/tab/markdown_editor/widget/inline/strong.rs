use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn text_format_strong(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);

        let family =
            if parent_text_format.font_id.family == FontFamily::Name(Arc::from("SansSuper")) {
                FontFamily::Name(Arc::from("BoldSuper"))
            } else if parent_text_format.font_id.family == FontFamily::Name(Arc::from("SansSub")) {
                FontFamily::Name(Arc::from("BoldSub"))
            } else {
                FontFamily::Name(Arc::from("Bold"))
            };
        TextFormat {
            font_id: FontId { family, ..parent_text_format.font_id },
            ..parent_text_format
        }
    }

    pub fn span_strong(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.circumfix_span(node, wrap, range)
    }

    pub fn show_strong(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        self.show_circumfix(ui, node, top_left, wrap, range)
    }
}
