use comrak::nodes::{AstNode, NodeShortCode};
use egui::{Pos2, Sense, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};

impl<'ast> MdRender {
    pub fn text_format_short_code(&self, parent: &AstNode<'_>) -> Format {
        self.text_format(parent)
    }

    pub fn span_short_code(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (Grapheme, Grapheme),
        node_short_code: &NodeShortCode,
    ) -> f32 {
        let reveal = self.node_revealed(node);
        if reveal {
            self.circumfix_span(node, wrap, range)
        } else if range.contains_range(&self.node_range(node), true, true) {
            self.span_override_section(wrap, &node_short_code.emoji, self.text_format(node))
        } else {
            0.
        }
    }

    pub fn show_short_code(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (Grapheme, Grapheme), node_short_code: &NodeShortCode,
    ) -> Response {
        let reveal = self.node_revealed(node);
        if reveal {
            self.show_circumfix(ui, node, top_left, wrap, range)
        } else if range.contains_range(&self.node_range(node), true, true) {
            self.show_override_section(
                ui,
                top_left,
                wrap,
                self.node_range(node),
                self.text_format(node),
                Some(&node_short_code.emoji),
                Sense::hover(),
            )
        } else {
            Response::default()
        }
    }
}
