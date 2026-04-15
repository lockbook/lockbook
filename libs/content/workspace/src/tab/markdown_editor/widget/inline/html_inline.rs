use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::resolvers::{EmbedResolver, LinkResolver};
use crate::tab::markdown_editor::MdLabel;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};
use crate::theme::palette_v2::ThemeExt as _;

pub const FOLD_TAG: &str = "<!-- {\"fold\":true} -->";

impl<'ast, E: EmbedResolver, L: LinkResolver> MdLabel<E, L> {
    pub fn text_format_html_inline(&self, parent: &AstNode<'_>) -> Format {
        Format {
            color: self.ctx.get_lb_theme().neutral_fg_secondary(),
            background: self.ctx.get_lb_theme().neutral_bg_secondary(),
            ..self.text_format_code_block(parent)
        }
    }

    pub fn span_html_inline(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
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

            let pre_span = self.text_pre_span(&tmp_wrap, &text_format);
            let mid_span = self.text_mid_span(&tmp_wrap, pre_span, text, text_format.clone());
            let post_span = self.text_post_span(&tmp_wrap, pre_span + mid_span, &text_format);

            tmp_wrap.offset += pre_span + mid_span + post_span;
        }

        tmp_wrap.offset - wrap.offset
    }

    pub fn show_html_inline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
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
