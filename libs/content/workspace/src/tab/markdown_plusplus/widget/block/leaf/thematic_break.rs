use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};

use crate::tab::markdown_plusplus::widget::utils::text_layout::Wrap;
use crate::tab::markdown_plusplus::widget::ROW_HEIGHT;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn height_thematic_break(&self) -> f32 {
        ROW_HEIGHT
    }

    pub fn show_thematic_break(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        let node_line = self.node_line(node, self.node_first_line(node));

        if self.node_intersects_selection(node) {
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                node_line,
                self.text_format_syntax(node),
                false,
            );
        } else {
            let rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
            );
        }
    }

    pub fn compute_bounds_thematic_break(&mut self, node: &'ast AstNode<'ast>) {
        let node_line = self.node_line(node, self.node_first_line(node));
        self.bounds.paragraphs.push(node_line);
    }
}
