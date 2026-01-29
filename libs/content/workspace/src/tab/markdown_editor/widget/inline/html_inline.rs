use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

pub const FOLD_TAG: &str = "<!-- {\"fold\":true} -->";

impl<'ast> Editor {
    pub fn text_format_html_inline(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat {
            color: self.theme.fg().neutral_tertiary,
            background: self.theme.bg().neutral_secondary,
            ..self.text_format_code_block(parent)
        }
    }

    pub fn span_html_inline(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let node_range = self.node_range(node).trim(&range);
        let reveal = self.node_intersects_selection(node);

        // html comments not rendered unless revealed
        let node_text = &self.buffer[node_range];
        if !self.node_intersects_selection(node)
            && node_text.starts_with("<!--")
            && node_text.ends_with("-->")
        {
            return 0.;
        }

        let text_format =
            if reveal { self.text_format_syntax(node) } else { self.text_format(node) };

        let mut tmp_wrap = wrap.clone();

        if !node_range.is_empty() {
            let pre_span = self.text_pre_span(&tmp_wrap, text_format.clone());
            let mid_span = self.text_mid_span(
                &tmp_wrap,
                pre_span,
                &self.buffer[node_range],
                text_format.clone(),
            );
            let post_span = self.text_post_span(&tmp_wrap, pre_span + mid_span, text_format);

            tmp_wrap.offset += pre_span + mid_span + post_span;
        }

        tmp_wrap.offset - wrap.offset
    }

    pub fn show_html_inline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let node_range = self.node_range(node).trim(&range);
        let reveal = self.node_intersects_selection(node);

        // html comments not rendered unless revealed
        let node_text = &self.buffer[node_range];
        if !reveal && node_text.starts_with("<!--") && node_text.ends_with("-->") {
            return Default::default();
        }

        let mut response = Default::default();

        if !node_range.is_empty() {
            let sense = self.sense_inline(ui, node);
            let text_format =
                if reveal { self.text_format_syntax(node) } else { self.text_format(node) };
            response |= self.show_override_section(
                ui,
                top_left,
                wrap,
                node_range,
                text_format,
                false,
                None,
                sense,
            );
        }

        response
    }
}
