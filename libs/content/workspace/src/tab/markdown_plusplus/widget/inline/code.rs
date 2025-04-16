use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_code(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat {
            color: self.theme.fg().accent_secondary,
            background: self.theme.bg().neutral_secondary,
            ..self.text_format_code_block(parent)
        }
    }

    pub fn span_code(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext) -> f32 {
        let sourcepos = node.data.borrow().sourcepos;
        let range = self.sourcepos_to_range(sourcepos);

        let infix_range = (range.start() + 1, range.end() - 1);
        let infix_span = self.span_text_line(wrap, infix_range, self.text_format(node));

        if self.node_intersects_selection(node) {
            let prefix_range = (range.start(), range.start() + 1);
            let prefix_span = self.span_text_line(wrap, prefix_range, self.text_format(node));
            let postfix_range = (range.start(), range.start() + 1);
            let postfix_span = self.span_text_line(wrap, postfix_range, self.text_format(node));
            prefix_span + infix_span + postfix_span
        } else {
            infix_span
        }
    }

    pub fn show_code(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        let sourcepos = node.data.borrow().sourcepos;
        let range = self.sourcepos_to_range(sourcepos);

        let prefix_range = (range.start(), range.start() + 1);
        let infix_range = (range.start() + 1, range.end() - 1);
        let postfix_range = (range.end() - 1, range.end());

        if self.node_intersects_selection(node) {
            self.show_text_line(
                ui,
                top_left,
                wrap,
                prefix_range,
                self.text_format_syntax(node),
                false,
            );
        }

        self.show_text_line(ui, top_left, wrap, infix_range, self.text_format(node), false);

        if self.node_intersects_selection(node) {
            self.show_text_line(
                ui,
                top_left,
                wrap,
                postfix_range,
                self.text_format_syntax(node),
                false,
            );
        }
    }
}
