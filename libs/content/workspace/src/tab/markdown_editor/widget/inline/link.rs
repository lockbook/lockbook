use std::sync::Arc;

use comrak::nodes::{AstNode, NodeLink};
use egui::{FontFamily, FontId, OpenUrl, Pos2, Sense, Stroke, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::theme::icons::Icon;

impl<'ast> Editor {
    pub fn text_format_link(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: self.theme.fg().blue,
            underline: Stroke { width: 1., color: self.theme.fg().blue },
            ..parent_text_format
        }
    }

    pub fn text_format_link_button(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        let link_text_format = self.text_format_link(parent);
        TextFormat {
            color: link_text_format.color,
            font_id: FontId {
                family: FontFamily::Name(Arc::from("Icons")),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn span_link(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let mut tmp_wrap = wrap.clone();
        tmp_wrap.offset += self.circumfix_span(node, &tmp_wrap, range);
        tmp_wrap.offset += self.span_override_section(
            &tmp_wrap,
            Icon::OPEN_IN_NEW.icon,
            self.text_format_link_button(node.parent().unwrap()),
        );
        tmp_wrap.offset - wrap.offset
    }

    pub fn show_link(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        node_link: &NodeLink, range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        // An inline link consists of a link text followed immediately by a left
        // parenthesis `(`, optional whitespace, an optional link destination,
        // an optional link title separated from the link destination by
        // whitespace, optional whitespace, and a right parenthesis `)`
        // https://github.github.com/gfm/#inline-link

        // Although link titles may span multiple lines, they may not contain a
        // blank line.
        // https://github.github.com/gfm/#link-title

        let mut response = self.show_circumfix(ui, node, top_left, wrap, range);
        response.hovered &= self.sense_inline(ui, node).click;
        response |= self.show_override_section(
            ui,
            top_left,
            wrap,
            self.node_range(node).end().into_range(),
            self.text_format_link_button(node.parent().unwrap()),
            false,
            Some(Icon::OPEN_IN_NEW.icon),
            Sense { click: true, drag: false, focusable: false },
        );
        if response.hovered {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }
        if response.clicked {
            ui.output_mut(|o| o.open_url = Some(OpenUrl::new_tab(&node_link.url)));
        }

        response
    }
}
