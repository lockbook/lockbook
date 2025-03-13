use comrak::nodes::NodeList;
use egui::{Context, Pos2, Ui};

use crate::tab::markdown_plusplus::widget::{Ast, Block};

pub struct List<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    _node: &'w NodeList,
}

impl<'a, 't, 'w> List<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeList) -> Self {
        Self { ast, _node: node }
    }
}

impl Block for List<'_, '_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        self.ast.show_block_children(width, top_left, ui);

        // debug
        // let rect = Rect::from_min_size(top_left, ui.min_rect().max - top_left);
        // ui.painter()
        //     .rect_stroke(rect, 2., egui::Stroke::new(1., self.ast.theme.fg().yellow));
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast.block_children_height(width, ctx)
    }
}
