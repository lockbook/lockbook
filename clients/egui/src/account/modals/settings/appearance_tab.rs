use eframe::egui;

use crate::settings::ThemeMode;
use crate::theme;
use crate::widgets::ToolBarVisibility;

use super::SettingsResponse;

impl super::SettingsModal {
    pub fn show_appearance_tab(&mut self, ui: &mut egui::Ui) -> Option<SettingsResponse> {
        ui.heading("Theme Mode:");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let s = &mut self.settings.write().unwrap();

            for (mode, name) in vec![
                (ThemeMode::System, "System"),
                (ThemeMode::Dark, "Dark"),
                (ThemeMode::Light, "Light"),
            ] {
                if ui.selectable_value(&mut s.theme_mode, mode, name).clicked() {
                    theme::apply_settings(s, ui.ctx());
                }
            }
        });

        ui.add_space(24.0);
        ui.heading("Markdown Toolbar:");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let s = &mut self.settings.write().unwrap();
            if ui
                .selectable_value(
                    &mut s.toolbar_visibility,
                    ToolBarVisibility::Maximized,
                    "Enabled",
                )
                .clicked()
            {
                return Some(SettingsResponse::ToggleToolbarVisibility(
                    ToolBarVisibility::Maximized,
                ));
            }
            if ui
                .selectable_value(
                    &mut s.toolbar_visibility,
                    ToolBarVisibility::Disabled,
                    "Disabled",
                )
                .clicked()
            {
                return Some(SettingsResponse::ToggleToolbarVisibility(
                    ToolBarVisibility::Disabled,
                ));
            }
            None
        })
        .inner
    }
}
