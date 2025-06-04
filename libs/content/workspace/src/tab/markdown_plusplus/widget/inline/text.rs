use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};

use crate::tab::markdown_plusplus::widget::utils::text_layout::Wrap;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn span_text(&self, node: &'ast AstNode<'ast>, wrap: &Wrap, text: &str) -> f32 {
        let text_format = self.text_format(node);

        let pre_span = self.text_pre_span(wrap, text_format.clone());
        let mid_span = self.text_mid_span(wrap, pre_span, text, text_format.clone());
        let post_span = self.text_post_span(wrap, pre_span + mid_span, text_format);

        pre_span + mid_span + post_span
    }

    pub fn show_text(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        let range = self.node_range(node);
        let text_format = self.text_format(node);
        let spoiler = node
            .ancestors()
            .any(|node| matches!(node.data.borrow().value, NodeValue::SpoileredText));

        self.show_text_line(ui, top_left, wrap, range, text_format, spoiler);
    }
}
