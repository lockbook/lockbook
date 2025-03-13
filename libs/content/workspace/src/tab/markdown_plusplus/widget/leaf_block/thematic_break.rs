use egui::{Context, Pos2, Rect, Stroke, Ui, Vec2};

use crate::tab::markdown_plusplus::widget::{Ast, Block, ROW_HEIGHT};

pub struct ThematicBreak<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
}

impl<'a, 't, 'w> ThematicBreak<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { ast }
    }
}

impl Block for ThematicBreak<'_, '_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        let rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));

        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            Stroke { width: 1.0, color: self.ast.theme.bg().neutral_tertiary },
        );

        // debug
        // ui.painter()
        //     .rect_stroke(rect, 2., egui::Stroke::new(1., self.ast.theme.bg().tertiary));
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        ROW_HEIGHT
    }
}
