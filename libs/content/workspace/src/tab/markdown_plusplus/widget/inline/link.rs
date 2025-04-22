use comrak::nodes::AstNode;
use egui::{Pos2, Stroke, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::Wrap, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_link(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: self.theme.fg().blue,
            underline: Stroke { width: 1., color: self.theme.fg().blue },
            ..parent_text_format
        }
    }

    pub fn span_link(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_link(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        // An inline link consists of a link text followed immediately by a left
        // parenthesis `(`, optional whitespace, an optional link destination,
        // an optional link title separated from the link destination by
        // whitespace, optional whitespace, and a right parenthesis `)`
        // https://github.github.com/gfm/#inline-link

        // Although link titles may span multiple lines, they may not contain a
        // blank line.
        // https://github.github.com/gfm/#link-title

        // self.show_inline_children(ui, node, top_left, wrap);

        self.show_circumfix(ui, node, top_left, wrap);
    }
}
