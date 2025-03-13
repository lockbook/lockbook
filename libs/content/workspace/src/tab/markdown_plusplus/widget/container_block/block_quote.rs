use egui::{Context, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Block, INDENT},
};

pub struct BlockQuote<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
}

impl<'a, 't, 'w> BlockQuote<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>) -> Self {
        Self { ast }
    }

    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        TextFormat { color: theme.fg().neutral_tertiary, ..parent_text_format }
    }
}

impl Block for BlockQuote<'_, '_, '_> {
    fn show(&self, mut width: f32, mut top_left: Pos2, ui: &mut Ui) {
        let height = self.height(width, ui.ctx());
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        ui.painter().vline(
            annotation_space.center().x,
            annotation_space.y_range(),
            Stroke::new(3., self.ast.theme.bg().neutral_tertiary),
        );

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.ast.theme.fg().blue));

        top_left.x += annotation_space.width();
        width -= annotation_space.width();
        self.ast.show_block_children(width, top_left, ui);
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast.block_children_height(width - INDENT, ctx)
    }
}
