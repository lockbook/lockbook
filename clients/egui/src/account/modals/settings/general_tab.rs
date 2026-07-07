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
                let mut contact_linked_sites = self.ws_persistent_store.get_contact_linked_sites();
                if switch(ui, &mut contact_linked_sites).changed() {
                    self.ws_persistent_store
                        .set_contact_linked_sites(contact_linked_sites);
                }
                ui.label("Fetch link previews");
            });

            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(
                    "Showing titles and preview cards means contacting the linked site, \
                     which reveals your IP address and that you opened the note. Off by default.",
                );
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                switch(ui, &mut s.sidebar_usage);
                ui.label("Show usage in sidebar");
            });

            #[cfg(target_os = "linux")]
            {
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    switch(ui, &mut s.allow_wayland);
                    ui.label("Allow Wayland (restart required)");
                });

                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Wayland enables fractional display scaling but disables drag-and-drop. See ");
                    ui.hyperlink_to(
                        "issue #4607",
                        "https://github.com/lockbook/lockbook/issues/4607",
                    );
                    ui.label(" for details.");
                });
            }
        });
    }
}
