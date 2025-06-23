use comrak::nodes::AstNode;
use egui::{Pos2, Stroke, TextFormat, Ui};

use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_underline(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            underline: Stroke { width: 1., color: parent_text_format.color },
            ..parent_text_format
        }
    }

    pub fn span_underline(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_underline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        self.show_circumfix(ui, node, top_left, wrap);
    }
}
