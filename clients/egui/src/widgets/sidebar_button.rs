use eframe::egui;

use crate::theme::Icon;

use super::Button;

pub fn sidebar_button(ui: &mut egui::Ui, icon: &Icon, text: &str) -> egui::Response {
    Button::default()
        .icon(icon)
        .text(text)
        .style(egui::TextStyle::Heading)
        .fill(ui.visuals().faint_bg_color)
        .padding((7.0, 9.0))
        .hexpand(true)
        .show(ui)
}
