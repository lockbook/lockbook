use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};

impl<'ast> MdRender {
    pub fn text_format_superscript(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { superscript: true, ..parent_text_format }
    }

    pub fn span_superscript(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let text_format_syntax = self.text_format_syntax();

        let mut tmp_wrap = wrap.clone();
        if self.node_revealed(node) {
            if let Some(prefix_range) = self.prefix_range(node) {
                if range.contains_range(&prefix_range, true, true) {
                    tmp_wrap.offset +=
                        self.span_section(wrap, prefix_range, text_format_syntax.clone());
                }
            }
        }
        tmp_wrap.offset += self.inline_children_span(node, &tmp_wrap, range);
        if self.node_revealed(node) {
            if let Some(postfix_range) = self.postfix_range(node) {
                if range.contains_range(&postfix_range, true, true) {
                    tmp_wrap.offset +=
                        self.span_section(wrap, postfix_range, text_format_syntax.clone());
                }
            }
        }
        tmp_wrap.offset - wrap.offset
    }

    pub fn show_superscript(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let text_format_syntax = self.text_format_syntax();

        let mut response = Default::default();

        if self.node_revealed(node) {
            if let Some(prefix_range) = self.prefix_range(node) {
                if range.contains_range(&prefix_range, true, true) {
                    response |= self.show_section(
                        ui,
                        top_left,
                        wrap,
                        prefix_range,
                        text_format_syntax.clone(),
                    );
                }
            }
        }

        response |= self.show_inline_children(ui, node, top_left, wrap, range);

        if self.node_revealed(node) {
            if let Some(postfix_range) = self.postfix_range(node) {
                if range.contains_range(&postfix_range, true, true) {
                    response |= self.show_section(
                        ui,
                        top_left,
                        wrap,
                        postfix_range,
                        text_format_syntax.clone(),
                    );
                }
            }
        }

        response
    }
}
