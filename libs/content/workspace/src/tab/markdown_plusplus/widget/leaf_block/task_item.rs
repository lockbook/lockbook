use egui::{Context, Pos2, Rect, Ui, Vec2};

use crate::tab::markdown_plusplus::widget::{Ast, Block, INDENT};

pub struct TaskItem<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    maybe_check: &'w Option<char>,
}

impl<'a, 't, 'w> TaskItem<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, maybe_check: &'w Option<char>) -> Self {
        Self { ast, maybe_check }
    }
}

impl Block for TaskItem<'_, '_, '_> {
    fn show(&self, mut width: f32, mut top_left: Pos2, ui: &mut Ui) {
        let space =
            Rect::from_min_size(top_left, Vec2 { x: INDENT, y: self.ast.row_height(ui.ctx()) });

        ui.allocate_ui_at_rect(space, |ui| ui.checkbox(&mut self.maybe_check.is_some(), ""));

        top_left.x += space.width();
        width -= space.width();
        self.ast.show_block_children(width, top_left, ui);

        // todo: add space for captured lines
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast.block_children_height(width, ctx)
    }
}
