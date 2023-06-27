use eframe::egui;

use crate::theme::Icon;

use super::Button;

pub fn sidebar_button(ui: &mut egui::Ui, icon: &Icon, text: &str) -> egui::Response {
    // Button::default()
    //     .icon(icon)
    //     .text(text)
    //     .style(egui::TextStyle::Heading)
    //     .padding((7.0, 9.0))
    //     .hexpand(true)
    //     .show(ui)

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_size_before_wrap().x, 30.0),
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.add_space(10.0);
            Icon::SETTINGS.show(ui);
            ui.add_space(20.0);
            Icon::SHARED_FOLDER.show(ui)
        },
    )
    .inner
}
