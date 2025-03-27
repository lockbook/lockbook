use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn show_table(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32,
    ) {
        self.show_block_children(ui, node, top_left, width);

        // draw exterior decoration
        let table = Rect::from_min_size(
            top_left,
            Vec2::new(width, self.block_children_height(node, width)),
        );
        ui.painter().rect_stroke(
            table,
            2.,
            Stroke { width: 1., color: self.theme.bg().neutral_tertiary },
        );
    }
}
