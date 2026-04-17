use comrak::nodes::AstNode;
use egui::{Pos2, Sense, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn span_text(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let node_range = self.node_range(node);
        let text_format = self.text_format(node);

        let pre_span = self.text_pre_span(wrap, &text_format);
        let mid_span = self.text_mid_span(
            wrap,
            pre_span,
            &self.buffer[node_range.trim(&range)],
            text_format.clone(),
        );
        let post_span = self.text_post_span(wrap, pre_span + mid_span, &text_format);

        pre_span + mid_span + post_span
    }

    pub fn show_text(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let node_range = self.node_range(node).trim(&range);
        let text_format = self.text_format(node);
        let sense = if self.inline_clickable(ui, node) { Sense::click() } else { Sense::hover() };

        if node_range.is_empty() {
            return Default::default();
        }

        if let Some(search_range) = self.text_highlight_range {
            let start = search_range.0.max(node_range.0);
            let end = search_range.1.min(node_range.1);
            if start < end {
                let theme = self.ctx.get_lb_theme();
                let accent = theme.fg().get_color(theme.prefs().primary);
                let accent_format = Format { color: accent, ..text_format.clone() };
                let mut resp = Response::default();
                if node_range.0 < start {
                    resp |= self.show_override_section(
                        ui,
                        top_left,
                        wrap,
                        (node_range.0, start),
                        text_format.clone(),
                        None,
                        sense,
                    );
                }
                resp |= self.show_override_section(
                    ui,
                    top_left,
                    wrap,
                    (start, end),
                    accent_format,
                    None,
                    sense,
                );
                if end < node_range.1 {
                    resp |= self.show_override_section(
                        ui,
                        top_left,
                        wrap,
                        (end, node_range.1),
                        text_format,
                        None,
                        sense,
                    );
                }
                return resp;
            }
        }

        self.show_override_section(ui, top_left, wrap, node_range, text_format, None, sense)
    }
}
