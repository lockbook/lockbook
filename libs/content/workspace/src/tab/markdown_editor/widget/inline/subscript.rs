use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::resolvers::{EmbedResolver, LinkResolver};
use crate::tab::markdown_editor::MdLabel;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};

impl<'ast, E: EmbedResolver, L: LinkResolver> MdLabel<E, L> {
    pub fn text_format_subscript(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { subscript: true, ..parent_text_format }
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
                    response |=
                        self.show_section(ui, top_left, wrap, postfix_range, text_format_syntax);
                }
            }
        }

        response
    }
}
