use eframe::egui;

pub fn switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);

    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, ""));

    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);

        let visuals = ui.style().interact_selectable(&response, *on);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();

        let (bg_fill, circle_fill) = if *on {
            (ui.visuals().widgets.active.bg_fill, ui.visuals().widgets.inactive.fg_stroke.color)
        } else {
            (ui.visuals().widgets.inactive.bg_fill, ui.visuals().faint_bg_color)
        };

        ui.painter().rect(rect, radius, bg_fill, visuals.bg_stroke);

        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);

        ui.painter()
            .circle(center, 0.75 * radius, circle_fill, visuals.fg_stroke);
    }

    response
}
