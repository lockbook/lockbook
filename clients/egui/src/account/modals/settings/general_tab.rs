use egui;
use workspace_rs::widgets::switch;

impl super::SettingsModal {
    pub fn show_general_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("General");
        ui.add_space(12.0);

        let s = &mut self.settings.write().unwrap();

        ui.group(|ui| {
            ui.horizontal(|ui| {
                switch(ui, &mut s.window_maximize);
                ui.label("Maximize window on startup");
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                switch(ui, &mut s.auto_sync);
                ui.label("Auto-sync");
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                switch(ui, &mut s.auto_save);
                ui.label("Auto-save");
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                switch(ui, &mut s.sidebar_usage);
                ui.label("Show usage in sidebar");
            });
        });
    }
}
