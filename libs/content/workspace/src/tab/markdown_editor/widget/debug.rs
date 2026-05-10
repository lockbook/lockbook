use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Ui, Vec2};
use web_time::Instant;

use crate::tab::markdown_editor::MdRender;
use crate::theme::palette_v2::ThemeExt as _;

impl MdRender {
    pub fn show_debug_metrics(&mut self, ui: &mut Ui) {
        let now = Instant::now();
        self.frame_times[self.frame_times_idx] = now;
        self.frame_times_idx = (self.frame_times_idx + 1) % self.frame_times.len();

        let oldest = self.frame_times[self.frame_times_idx];
        let elapsed = now.duration_since(oldest).as_secs_f32();
        let fps = if elapsed > 0.0 { self.frame_times.len() as f32 / elapsed } else { 0.0 };

        let latency_text = match (
            self.ime_replace_latency.last_ms(),
            self.ime_replace_latency.p50_ms(),
            self.ime_replace_latency.p90_ms(),
        ) {
            (Some(last), Some(p50), Some(p90)) => {
                format!("ime->editor last {:.1} ms\np50 {:.1} ms\np90 {:.1} ms", last, p50, p90)
            }
            _ => "ime->editor no samples yet".to_string(),
        };
        let debug_text = format!("{:.0} fps\n{}", fps, latency_text);
        let rect = ui.max_rect();
        let pos = rect.right_top() + Vec2::new(-140., 5.);
        ui.painter().text(
            pos,
            egui::Align2::RIGHT_TOP,
            debug_text,
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
