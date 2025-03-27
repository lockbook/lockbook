use egui::{Pos2, Rect, Stroke, Ui, Vec2};

use crate::tab::markdown_plusplus::{widget::ROW_HEIGHT, MarkdownPlusPlus};

impl MarkdownPlusPlus {
    pub fn height_thematic_break(&self) -> f32 {
        ROW_HEIGHT
    }

    pub fn show_thematic_break(&mut self, ui: &mut Ui, top_left: Pos2, width: f32) {
        let rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));

        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
        );
    }
}
