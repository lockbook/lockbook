use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RelCharOffset};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, INDENT, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn height_task_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_item(node)
    }

    pub fn show_task_item(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        maybe_check: Option<char>,
    ) {
        let space = Rect::from_min_size(top_left, Vec2 { x: INDENT, y: ROW_HEIGHT });

        ui.allocate_ui_at_rect(space, |ui| ui.checkbox(&mut maybe_check.is_some(), ""));

        top_left.x += space.width();
        self.show_block_children(ui, node, top_left);

        // todo: add space for captured lines
    }

    pub fn line_prefix_len_task_item(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> RelCharOffset {
        todo!()
    }

    pub fn show_line_prefix_task_item(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        top_left: Pos2, height: f32, row_height: f32,
    ) {
        todo!()
    }
}
