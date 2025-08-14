use comrak::nodes::AstNode;
use egui::{Pos2, Sense, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;

impl<'ast> Editor {
    pub fn text_format_footnote_reference(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat {
            color: self.theme.fg().neutral_tertiary,
            ..self.text_format_superscript(parent)
        }
    }

    pub fn span_footnote_reference(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, ix: u32,
        range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let node_range = self.node_range(node);

        if self.node_intersects_selection(node) {
            let prefix_range = (node_range.start(), node_range.start() + 2);
            let infix_range = (node_range.start() + 2, node_range.end() - 1);
            let postfix_range = (node_range.end() - 1, node_range.end());

            let mut span = 0.0;

            if range.contains_range(&prefix_range, true, true) {
                span += self.span_text_line(wrap, prefix_range, self.text_format_syntax(node));
            }
            if range.contains_range(&infix_range, true, true) {
                span += self.span_text_line(wrap, infix_range, self.text_format(node));
            }
            if range.contains_range(&postfix_range, true, true) {
                span += self.span_text_line(wrap, postfix_range, self.text_format_syntax(node));
            }

            span
        } else {
            let node_range = self.node_range(node);
            if range.contains_range(&node_range, true, true) {
                let text = format!("{ix}");
                self.text_mid_span(wrap, Default::default(), &text, self.text_format(node))
            } else {
                0.0
            }
        }
    }

    // [^footnotereference]
    pub fn show_footnote_reference(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        ix: u32, range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let node_range = self.node_range(node);

        let prefix_range = (node_range.start(), node_range.start() + 2);
        let infix_range = (node_range.start() + 2, node_range.end() - 1);
        let postfix_range = (node_range.end() - 1, node_range.end());

        let mut response = Default::default();

        if self.node_intersects_selection(node) {
            if range.contains_range(&prefix_range, true, true) {
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    prefix_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
            if range.contains_range(&infix_range, true, true) {
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    infix_range,
                    self.text_format(node),
                    false,
                );
            }
            if range.contains_range(&postfix_range, true, true) {
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    postfix_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
        } else {
            let node_range = self.node_range(node);
            if range.contains_range(&node_range, true, true) {
                let text = format!("{ix}");
                response |= self.show_override_text_line(
                    ui,
                    top_left,
                    wrap,
                    (node_range.end() - 1).into_range(),
                    self.text_format(node),
                    false,
                    Some(&text),
                    Sense { click: false, drag: false, focusable: false },
                );
            }
        }

        response
    }
}
