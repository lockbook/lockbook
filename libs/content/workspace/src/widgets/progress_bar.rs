pub struct ProgressBar {
    height: f32,
    percent: f32,
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressBar {
    pub fn new() -> Self {
        Self { height: 8.0, percent: 0.0 }
    }

    pub fn percent(self, percent: f32) -> Self {
        Self { percent, ..self }
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = egui::vec2(ui.available_width(), self.height);

        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            let rounding = egui::Rounding::same(self.height / 2.0);
            let stroke = egui::Stroke::NONE;

            // Background (the full line).
            ui.painter().add(epaint::RectShape {
                rect,
                rounding,
                fill: ui.visuals().extreme_bg_color,
                stroke,
            });

            let width = rect.max.x - rect.min.x;
            let mut progress_rect = rect;
            progress_rect.max.x = progress_rect.min.x + width * self.percent;

            // Filled portion.
            ui.painter().add(epaint::RectShape {
                rect: progress_rect,
                rounding,
                fill: ui.visuals().widgets.active.bg_fill,
                stroke,
            });
        }

        resp
    }
}
