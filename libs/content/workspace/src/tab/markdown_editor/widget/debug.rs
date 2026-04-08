use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Ui, Vec2};
use web_time::Instant;

use crate::tab::markdown_editor::Editor;
use crate::theme::palette_v2::ThemeExt as _;

impl Editor {
    pub fn show_debug_fps(&mut self, ui: &mut Ui) {
        let now = Instant::now();
        self.frame_times[self.frame_times_idx] = now;
        self.frame_times_idx = (self.frame_times_idx + 1) % self.frame_times.len();

        let oldest = self.frame_times[self.frame_times_idx];
        let elapsed = now.duration_since(oldest).as_secs_f32();
        let fps = if elapsed > 0.0 { self.frame_times.len() as f32 / elapsed } else { 0.0 };

        let fps_text = format!("{:.0} fps", fps);
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
