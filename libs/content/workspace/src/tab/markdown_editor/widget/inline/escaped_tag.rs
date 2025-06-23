use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_escaped_tag(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format(parent)
    }

    pub fn span_escaped_tag(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        let mut tmp_wrap = wrap.clone();

        let any_children = node.children().next().is_some();
        if any_children {
            tmp_wrap.offset += self.prefix_span(node, &tmp_wrap);
            tmp_wrap.offset += self.inline_children_span(node, &tmp_wrap);
            tmp_wrap.offset += self.postfix_span(node, &tmp_wrap);
        } else {
            tmp_wrap.offset +=
                self.span_text_line(wrap, self.node_range(node), self.text_format(node))
        }

        tmp_wrap.offset - wrap.offset
    }

    pub fn show_escaped_tag(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) -> Response {
        let mut response = Response { clicked: false, hovered: false };
        let any_children = node.children().next().is_some();
        if any_children {
            if let Some(prefix_range) = self.prefix_range(node) {
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    prefix_range,
                    self.text_format(node),
                    false,
                );
            }
            response |= self.show_inline_children(ui, node, top_left, wrap);
            if let Some(postfix_range) = self.postfix_range(node) {
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    postfix_range,
                    self.text_format(node),
                    false,
                );
            }
        } else {
            response |= self.show_text_line(
                ui,
                top_left,
                wrap,
                self.node_range(node),
                self.text_format(node),
                false,
            );
        }
        response
    }
}
