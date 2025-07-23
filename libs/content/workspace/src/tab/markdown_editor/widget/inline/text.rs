use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;

impl<'ast> Editor {
    pub fn span_text(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let node_range = self.node_range(node);
        let text_format = self.text_format(node);

        let pre_span = self.text_pre_span(wrap, text_format.clone());
        let mid_span = self.text_mid_span(
            wrap,
            pre_span,
            &self.buffer[node_range.trim(&range)],
            text_format.clone(),
        );
        let post_span = self.text_post_span(wrap, pre_span + mid_span, text_format);

        pre_span + mid_span + post_span
    }

    pub fn show_text(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let node_range = self.node_range(node).trim(&range);
        let text_format = self.text_format(node);
        let spoiler = node
            .ancestors()
            .any(|node| matches!(node.data.borrow().value, NodeValue::SpoileredText));
        let sense = self.sense_inline(ui, node);

        if !node_range.is_empty() {
            self.show_override_text_line(
                ui,
                top_left,
                wrap,
                node_range.trim(&range),
                text_format,
                spoiler,
                None,
                sense,
            )
        } else {
            Default::default()
        }
    }
}
