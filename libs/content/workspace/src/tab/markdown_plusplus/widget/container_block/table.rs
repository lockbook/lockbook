use comrak::nodes::NodeTable;
use egui::{Context, Pos2, Rect, Stroke, Ui, Vec2};

use crate::tab::markdown_plusplus::widget::{Ast, Block};

pub struct Table<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    _node: &'w NodeTable,
}

impl<'a, 't, 'w> Table<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeTable) -> Self {
        Self { ast, _node: node }
    }
}

impl Block for Table<'_, '_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        self.ast.show_block_children(width, top_left, ui);

        // draw exterior decoration
        let table = Rect::from_min_size(
            top_left,
            Vec2::new(width, self.ast.block_children_height(width, ui.ctx())),
        );
        ui.painter().rect_stroke(
            table,
            2.,
            Stroke { width: 1., color: self.ast.theme.bg().neutral_tertiary },
        );
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast.block_children_height(width, ctx)
    }
}
