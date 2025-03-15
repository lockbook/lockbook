use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    widget::{INDENT, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn height_task_item(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        self.height_item(node, width)
    }

    pub fn show_task_item(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, mut width: f32,
        maybe_check: Option<char>,
    ) {
        let space = Rect::from_min_size(top_left, Vec2 { x: INDENT, y: ROW_HEIGHT });

        ui.allocate_ui_at_rect(space, |ui| ui.checkbox(&mut maybe_check.is_some(), ""));

        top_left.x += space.width();
        width -= space.width();
        self.show_block_children(ui, node, top_left, width);

        // todo: add space for captured lines
    }
}
