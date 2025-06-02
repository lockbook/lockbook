use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::Wrap, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
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
    ) {
        let any_children = node.children().next().is_some();
        if any_children {
            if let Some(prefix_range) = self.prefix_range(node) {
                self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    prefix_range,
                    self.text_format(node),
                    false,
                );
            }
            self.show_inline_children(ui, node, top_left, wrap);
            if let Some(postfix_range) = self.postfix_range(node) {
                self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    postfix_range,
                    self.text_format(node),
                    false,
                );
            }
        } else {
            #[allow(clippy::collapsible_else_if)]
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
}
