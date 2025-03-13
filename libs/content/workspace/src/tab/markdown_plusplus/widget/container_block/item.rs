use std::sync::Arc;

use comrak::nodes::{ListDelimType, ListType, NodeList};
use egui::text::LayoutJob;
use egui::{Context, FontFamily, Pos2, Rect, Ui, Vec2};

use crate::tab::markdown_plusplus::widget::{Ast, Block, BULLET_RADIUS, INDENT};

pub struct Item<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeList,
}

impl<'a, 't, 'w> Item<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeList) -> Self {
        Self { ast, node }
    }
}

impl Block for Item<'_, '_, '_> {
    fn show(&self, mut width: f32, mut top_left: Pos2, ui: &mut Ui) {
        let height = self.height(width, ui.ctx());
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        match self.node.list_type {
            ListType::Bullet => {
                ui.painter().circle_filled(
                    annotation_space.center(),
                    BULLET_RADIUS,
                    self.ast.theme.fg().neutral_tertiary,
                );
            }
            ListType::Ordered => {
                let mut text_format = self.ast.text_format.clone();
                text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
                text_format.color = self.ast.theme.fg().neutral_tertiary;

                let text = match self.node.delimiter {
                    ListDelimType::Period => format!("{}.", self.node.start),
                    ListDelimType::Paren => format!("{})", self.node.start),
                };
                let layout_job = LayoutJob::single_section(text, text_format);
                let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                ui.painter()
                    .galley(annotation_space.left_top(), galley, Default::default());
            }
        }

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
