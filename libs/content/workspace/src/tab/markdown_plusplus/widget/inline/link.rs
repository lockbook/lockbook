use comrak::nodes::AstNode;
use egui::{Pos2, Stroke, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_link(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: self.theme.fg().blue,
            underline: Stroke { width: 1., color: self.theme.fg().blue },
            ..parent_text_format
        }
    }

    pub fn inline_span_link(&self, node: &AstNode<'_>, wrap: &WrapContext, title: &str) -> f32 {
        self.inline_span_text(node, wrap, title)
    }

    pub fn show_link(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
        title: &str,
    ) {
        self.show_text(ui, node, top_left, wrap, title);
    }
}
