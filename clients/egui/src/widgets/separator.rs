use eframe::egui;

pub fn separator(ui: &mut egui::Ui) {
    let is_horizontal_line = !ui.layout().main_dir().is_horizontal();

    let available_space = ui.available_size_before_wrap();
    let spacing = 1.0;

    let size = if is_horizontal_line {
        egui::vec2(available_space.x, spacing)
    } else {
        egui::vec2(spacing, available_space.y)
    };

    let (rect, response) = ui.allocate_at_least(size, egui::Sense::hover());

    if ui.is_rect_visible(response.rect) {
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;

        if is_horizontal_line {
            ui.painter().hline(rect.x_range(), rect.center().y, stroke);
        } else {
            ui.painter().vline(rect.center().x, rect.y_range(), stroke);
        }
    }
}
