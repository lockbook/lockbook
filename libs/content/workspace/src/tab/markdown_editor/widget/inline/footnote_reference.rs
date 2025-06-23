use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_footnote_reference(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat {
            color: self.theme.fg().neutral_tertiary,
            ..self.text_format_superscript(parent)
        }
    }

    pub fn span_footnote_reference(&self, node: &'ast AstNode<'ast>, wrap: &Wrap, ix: u32) -> f32 {
        let range = self.node_range(node);

        if self.node_intersects_selection(node) {
            let prefix_range = (range.start(), range.start() + 2);
            let prefix_span =
                self.span_text_line(wrap, prefix_range, self.text_format_syntax(node));
            let infix_range = (range.start() + 2, range.end() - 1);
            let infix_span = self.span_text_line(wrap, infix_range, self.text_format(node));
            let postfix_range = (range.end() - 1, range.end());
            let postfix_span =
                self.span_text_line(wrap, postfix_range, self.text_format_syntax(node));
            prefix_span + infix_span + postfix_span
        } else {
            let text = format!("{}", ix);
            self.text_mid_span(wrap, Default::default(), &text, self.text_format(node))
        }
    }

    // [^footnotereference]
    pub fn show_footnote_reference(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap, ix: u32,
    ) {
        let range = self.node_range(node);

        let prefix_range = (range.start(), range.start() + 2);
        let infix_range = (range.start() + 2, range.end() - 1);
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

            self.show_text_line(ui, top_left, wrap, infix_range, self.text_format(node), false);

            self.show_text_line(
                ui,
                top_left,
                wrap,
                postfix_range,
                self.text_format_syntax(node),
                false,
            );
        } else {
            let text = format!("{}", ix);
            self.show_override_text_line(
                ui,
                top_left,
                wrap,
                (range.end() - 1).into_range(),
                self.text_format(node),
                false,
                Some(&text),
            );
        }
    }
}
