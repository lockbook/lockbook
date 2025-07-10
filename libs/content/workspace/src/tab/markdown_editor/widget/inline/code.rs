use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_code(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat {
            color: self.theme.fg().accent_secondary,
            background: self.theme.bg().neutral_secondary,
            ..self.text_format_code_block(parent)
        }
    }

    pub fn span_code(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let node_range = self.node_range(node);

        let prefix_range = (node_range.start(), node_range.start() + 1);
        let infix_range = (node_range.start() + 1, node_range.end() - 1);
        let postfix_range = (node_range.end() - 1, node_range.end());

        let reveal = self.node_intersects_selection(node);
        let mut span = 0.0;

        if reveal {
            span +=
                self.span_text_line(wrap, prefix_range.trim(&range), self.text_format_syntax(node));
        }
        let infix_range_trim = infix_range.trim(&range);
        if !infix_range_trim.is_empty() {
            span += self.span_text_line(wrap, infix_range_trim, self.text_format(node));
        }
        if reveal {
            span += self.span_text_line(
                wrap,
                postfix_range.trim(&range),
                self.text_format_syntax(node),
            );
        }

        span
    }

    pub fn show_code(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let node_range = self.node_range(node);

        let prefix_range = (node_range.start(), node_range.start() + 1).trim(&range);
        let infix_range = (node_range.start() + 1, node_range.end() - 1).trim(&range);
        let postfix_range = (node_range.end() - 1, node_range.end()).trim(&range);

        let reveal = self.node_intersects_selection(node);
        let mut response = Default::default();

        if !prefix_range.is_empty() {
            // prefix range is empty when it's trimmed to 0 because we're not
            // rendering the line containing the prefix
            if reveal {
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    prefix_range,
                    self.text_format_syntax(node),
                    false,
                );
            } else {
                // when syntax is captured, show an empty range
                // representing the beginning of the prefix, so that clicking
                // at the start of the circumfix places the cursor before
                // the syntax
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    prefix_range.start().into_range(),
                    self.text_format_syntax(node),
                    false,
                );
            }
        }
        if !infix_range.is_empty() {
            response |=
                self.show_text_line(ui, top_left, wrap, infix_range, self.text_format(node), false);
        }
        if !postfix_range.is_empty() {
            // postfix range is empty when it's trimmed to 0 because we're not
            // rendering the line containing the postfix
            if reveal {
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    postfix_range.trim(&range),
                    self.text_format_syntax(node),
                    false,
                );
            } else {
                // when syntax is captured, show an empty range
                // representing the end of the postfix, so that clicking
                // at the end of the circumfix places the cursor after
                // the syntax
                response |= self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    postfix_range.end().into_range(),
                    self.text_format_syntax(node),
                    false,
                );
            }
        }

        response
    }
}
