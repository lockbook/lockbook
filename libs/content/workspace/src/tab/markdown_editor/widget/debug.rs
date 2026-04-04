use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Ui, Vec2};
use web_time::Instant;

use crate::tab::markdown_editor::Editor;
use crate::theme::palette_v2::ThemeExt as _;

impl Editor {
    pub fn show_debug_fps(&mut self, ui: &mut Ui) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        // exponential moving average for smooth display
        self.fps = self.fps * 0.9 + (1.0 / dt) * 0.1;

        let fps_text = format!("{:.0} fps", self.fps);
        let rect = ui.max_rect();
        let pos = rect.right_top() + Vec2::new(-60., 5.);
        ui.painter().text(
            pos,
            egui::Align2::RIGHT_TOP,
            fps_text,
            egui::FontId::monospace(14.),
            self.ctx
                .get_lb_theme()
                .fg()
                .get_color(self.ctx.get_lb_theme().prefs().primary),
        );
    }

    pub fn show_debug_block_highlight<'ast>(
        &self, ui: &mut Ui, child: &'ast AstNode<'ast>, top_left: Pos2, width: f32, height: f32,
    ) {
        let child_rect = Rect::from_min_size(top_left, Vec2 { x: width, y: height });

        if self.selected_block(child) {
            ui.painter().rect(
                child_rect,
                2.,
                self.ctx.get_lb_theme().neutral_bg_secondary(),
                egui::Stroke { width: 1., color: self.ctx.get_lb_theme().neutral_bg_tertiary() },
                egui::epaint::StrokeKind::Inside,
            );
        }
    }
}
