use std::sync::Arc;

use comrak::nodes::{AstNode, ListType};
use egui::text::LayoutJob;
use egui::{FontFamily, Pos2, Rect, Ui, Vec2};

use crate::tab::markdown_plusplus::widget::{BULLET_RADIUS, INDENT, ROW_HEIGHT};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn height_item(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        self.block_children_height(node, width - INDENT)
    }

    pub fn show_item(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, mut width: f32,
        list_type: ListType, start: usize,
    ) {
        // todo: better bullet position for headings in list items
        let annotation_size = Vec2 { x: INDENT, y: ROW_HEIGHT };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        match list_type {
            ListType::Bullet => {
                ui.painter().circle_filled(
                    annotation_space.center(),
                    BULLET_RADIUS,
                    self.theme.fg().neutral_tertiary,
                );
            }
            ListType::Ordered => {
                let mut text_format = self.text_format(node);
                text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
                text_format.color = self.theme.fg().neutral_tertiary;

                let text = format!("{}.", start);
                let layout_job = LayoutJob::single_section(text, text_format);
                let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                ui.painter()
                    .galley(annotation_space.left_top(), galley, Default::default());
            }
        }

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));

        top_left.x += annotation_space.width();
        width -= annotation_space.width();
        self.show_block_children(ui, node, top_left, width);
    }
}
