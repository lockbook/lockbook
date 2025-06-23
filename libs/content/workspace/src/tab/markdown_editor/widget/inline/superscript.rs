use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_superscript(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        let parent_row_height = self.row_height(parent);
        TextFormat {
            font_id: FontId {
                family: FontFamily::Name(Arc::from("Super")),
                ..parent_text_format.font_id
            },
            line_height: Some(parent_row_height),
            ..parent_text_format
        }
    }

    pub fn span_superscript(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        let mut text_format_syntax = self.text_format_syntax(node);
        text_format_syntax.font_id.size = self.text_format(node.parent().unwrap()).font_id.size;

        let mut tmp_wrap = wrap.clone();
        if self.node_intersects_selection(node) {
            tmp_wrap.offset += if let Some(prefix_range) = self.prefix_range(node) {
                self.span_text_line(wrap, prefix_range, text_format_syntax.clone())
            } else {
                0.
            };
        }
        tmp_wrap.offset += self.inline_children_span(node, &tmp_wrap);
        if self.node_intersects_selection(node) {
            tmp_wrap.offset += if let Some(postfix_range) = self.postfix_range(node) {
                self.span_text_line(wrap, postfix_range, text_format_syntax)
            } else {
                0.
            };
        }
        tmp_wrap.offset - wrap.offset
    }

    pub fn show_superscript(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        let mut text_format_syntax = self.text_format_syntax(node);
        text_format_syntax.font_id.size = self.text_format(node.parent().unwrap()).font_id.size;

        if self.node_intersects_selection(node) {
            if let Some(prefix_range) = self.prefix_range(node) {
                self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    prefix_range,
                    text_format_syntax.clone(),
                    false,
                );
            }
        }

        self.show_inline_children(ui, node, top_left, wrap);

        if self.node_intersects_selection(node) {
            if let Some(postfix_range) = self.postfix_range(node) {
                self.show_text_line(ui, top_left, wrap, postfix_range, text_format_syntax, false);
            }
        }
    }
}
