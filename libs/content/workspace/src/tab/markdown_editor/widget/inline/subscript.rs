use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn text_format_subscript(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        let parent_row_height = self.row_height(parent);
        TextFormat {
            font_id: FontId {
                family: FontFamily::Name(Arc::from("Sub")),
                ..parent_text_format.font_id
            },
            line_height: Some(parent_row_height),
            ..parent_text_format
        }
    }

    pub fn span_subscript(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.span_superscript(node, wrap, range)
    }

    pub fn show_subscript(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let mut text_format_syntax = self.text_format_syntax(node);
        text_format_syntax.font_id.size = self.text_format(node.parent().unwrap()).font_id.size;

        let mut response = Default::default();

        if self.node_intersects_selection(node) {
            if let Some(prefix_range) = self.prefix_range(node) {
                if range.contains_range(&prefix_range, true, true) {
                    response |= self.show_section(
                        ui,
                        top_left,
                        wrap,
                        prefix_range,
                        text_format_syntax.clone(),
                        false,
                    );
                }
            }
        }

        response |= self.show_inline_children(ui, node, top_left, wrap, range);

        if self.node_intersects_selection(node) {
            if let Some(postfix_range) = self.postfix_range(node) {
                if range.contains_range(&postfix_range, true, true) {
                    response |= self.show_section(
                        ui,
                        top_left,
                        wrap,
                        postfix_range,
                        text_format_syntax,
                        false,
                    );
                }
            }
        }

        response
    }
}
