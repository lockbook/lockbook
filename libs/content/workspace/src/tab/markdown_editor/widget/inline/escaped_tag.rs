use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn text_format_escaped_tag(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format(parent)
    }

    pub fn span_escaped_tag(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let mut tmp_wrap = wrap.clone();

        let any_children = node.children().next().is_some();
        if any_children {
            if let Some(prefix_range) = self.prefix_range(node) {
                if range.contains_range(&prefix_range, true, true) {
                    tmp_wrap.offset += self.prefix_span(node, &tmp_wrap);
                }
            }
            tmp_wrap.offset += self.inline_children_span(node, &tmp_wrap, range);
            if let Some(postfix_range) = self.postfix_range(node) {
                if range.contains_range(&postfix_range, true, true) {
                    tmp_wrap.offset += self.postfix_span(node, &tmp_wrap);
                }
            }
        } else {
            let node_range = self.node_range(node);
            if range.contains_range(&node_range, true, true) {
                tmp_wrap.offset += self.span_section(wrap, node_range, self.text_format(node))
            }
        }

        tmp_wrap.offset - wrap.offset
    }

    pub fn show_escaped_tag(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let mut response = Default::default();
        let any_children = node.children().next().is_some();
        if any_children {
            if let Some(prefix_range) = self.prefix_range(node) {
                if range.contains_range(&prefix_range, true, true) {
                    response |= self.show_section(
                        ui,
                        top_left,
                        wrap,
                        prefix_range,
                        self.text_format(node),
                        false,
                    );
                }
            }
            response |= self.show_inline_children(ui, node, top_left, wrap, range);
            if let Some(postfix_range) = self.postfix_range(node) {
                if range.contains_range(&postfix_range, true, true) {
                    response |= self.show_section(
                        ui,
                        top_left,
                        wrap,
                        postfix_range,
                        self.text_format(node),
                        false,
                    );
                }
            }
        } else {
            let node_range = self.node_range(node);
            if range.contains_range(&node_range, true, true) {
                response |= self.show_section(
                    ui,
                    top_left,
                    wrap,
                    node_range,
                    self.text_format(node),
                    false,
                );
            }
        }
        response
    }
}
