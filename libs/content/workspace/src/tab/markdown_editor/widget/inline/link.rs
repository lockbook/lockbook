use comrak::nodes::{AstNode, NodeLink};
use egui::{OpenUrl, Pos2, Stroke, TextFormat, Ui};
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_link(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: self.theme.fg().blue,
            underline: Stroke { width: 1., color: self.theme.fg().blue },
            ..parent_text_format
        }
    }

    pub fn span_link(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.circumfix_span(node, wrap, range)
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

        let response = self.show_circumfix(ui, node, top_left, wrap, range);

        let modifiers = ui.input(|i| i.modifiers);
        if response.hovered && modifiers.command {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }
        if response.clicked && modifiers.command {
            let url = &node_link.url;
            let url = if !url.contains("://") { &format!("https://{}", url) } else { url };
            ui.output_mut(|o| o.open_url = Some(OpenUrl::new_tab(url)));
        }

        response
    }
}
