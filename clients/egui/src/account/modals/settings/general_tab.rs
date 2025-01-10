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
                let mut auto_sync = self.ws_persistent_store.get_auto_sync();
                if switch(ui, &mut auto_sync).changed() {
                    self.ws_persistent_store.set_auto_sync(auto_sync);
                }
                ui.label("Auto-sync");
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                let mut auto_save = self.ws_persistent_store.get_auto_save();
                if switch(ui, &mut auto_save).changed() {
                    self.ws_persistent_store.set_auto_save(auto_save);
                }
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
