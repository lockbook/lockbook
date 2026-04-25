use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};
use crate::theme::palette_v2::ThemeExt as _;

pub const FOLD_TAG: &str = "<!-- {\"fold\":true} -->";

impl<'ast> MdRender {
    pub fn text_format_html_inline(&self, parent: &AstNode<'_>) -> Format {
        Format {
            color: self.ctx.get_lb_theme().neutral_fg_secondary(),
            background: self.ctx.get_lb_theme().neutral_bg_secondary(),
            ..self.text_format_code_block(parent)
        }
    }

    pub fn span_html_inline(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (Grapheme, Grapheme),
    ) -> f32 {
        let node_range = self.node_range(node).trim(&range);
        let selection = self.buffer.current.selection;
        let reveal = node_range.contains_inclusive(selection.start())
            || node_range.contains_inclusive(selection.end())
            || self.foldee(node).is_none();

        let mut tmp_wrap = wrap.clone();

        if !node_range.is_empty() {
            let text_format =
                if reveal { self.text_format_syntax() } else { self.text_format(node) };
            let text = if reveal { &self.buffer[node_range] } else { "" };
            tmp_wrap.offset += self.span_override_section(&tmp_wrap, text, text_format);
        }

        tmp_wrap.offset - wrap.offset
    }

    pub fn show_html_inline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (Grapheme, Grapheme),
    ) -> Response {
        let node_range = self.node_range(node).trim(&range);
        let selection = self.buffer.current.selection;
        let reveal = node_range.contains_inclusive(selection.start())
            || node_range.contains_inclusive(selection.end())
            || self.foldee(node).is_none();

        let mut response = Default::default();

        if !node_range.is_empty() {
            let sense = if self.inline_clickable(ui, node) {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let text_format =
                if reveal { self.text_format_syntax() } else { self.text_format(node) };
            let override_text = if reveal { None } else { Some("") };

            response |= self.show_override_section(
                ui,
                top_left,
                wrap,
                node_range,
                text_format,
                override_text,
                sense,
            );
        }

        response
    }
}
