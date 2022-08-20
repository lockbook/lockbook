use std::sync::{Arc, RwLock};

use eframe::egui;
use egui_extras::{Size, StripBuilder};
use egui_winit::clipboard::Clipboard;

use crate::settings::{Settings, ThemeMode};
use crate::theme;
use crate::widgets::{subscription, switch};

enum SettingsTab {
    Account,
    Subscription,
    Appearance,
    General,
}

struct AccountSettings {
    export_result: Result<String, String>,
}

struct UsageSettings {
    sub_info_result: Result<Option<lb::SubscriptionInfo>, String>,
    metrics_result: Result<lb::UsageMetrics, String>,
    uncompressed_result: Result<lb::UsageItemMetric, String>,
}

pub struct SettingsModal {
    //core: Arc<lb::Core>,
    settings: Arc<RwLock<Settings>>,
    account: AccountSettings,
    usage: UsageSettings,
    active_tab: SettingsTab,
}

impl SettingsModal {
    pub fn open(core: &Arc<lb::Core>, s: &Arc<RwLock<Settings>>) -> Option<Box<Self>> {
        let export_result = core.export_account().map_err(|err| format!("{:?}", err)); // TODO

        let sub_info_result = core
            .get_subscription_info()
            .map_err(|err| format!("{:?}", err)); // TODO

        let metrics_result = core.get_usage().map_err(|err| format!("{:?}", err)); // TODO
        let uncompressed_result = core
            .get_uncompressed_usage()
            .map_err(|err| format!("{:?}", err)); // TODO

        Some(Box::new(Self {
            //core: core.clone(),
            settings: s.clone(),
            account: AccountSettings { export_result },
            usage: UsageSettings { sub_info_result, metrics_result, uncompressed_result },
            active_tab: SettingsTab::Account,
        }))
    }

    fn draw_tab_labels(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            if ui.button("Account").clicked() {
                self.active_tab = SettingsTab::Account;
            }
            if ui.button("Subscription").clicked() {
                self.active_tab = SettingsTab::Subscription;
            }
            if ui.button("Appearance").clicked() {
                self.active_tab = SettingsTab::Appearance;
            }
            if ui.button("General").clicked() {
                self.active_tab = SettingsTab::General;
            }
        });
    }

    fn draw_account_tab(&mut self, ui: &mut egui::Ui) {
        match &self.account.export_result {
            Ok(key) => {
                ui.heading("Export Account");
                ui.add_space(12.0);

                ui.label(EXPORT_DESC);
                ui.add_space(12.0);

                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::remainder())
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            if ui.button("Copy to Clipboard").clicked() {
                                Clipboard::default().set(key.to_owned());
                            }
                        });
                        strip.cell(|ui| {
                            if ui.button("Show QR Code").clicked() {
                                println!("show qr");
                            }
                        });
                    });
            }
            Err(err) => {
                ui.label(err);
            }
        };
    }

    fn draw_subscription_tab(&mut self, ui: &mut egui::Ui) {
        let metrics = match &self.usage.metrics_result {
            Ok(m) => m,
            Err(err) => {
                ui.label(err);
                return;
            }
        };

        let uncompressed = match &self.usage.uncompressed_result {
            Ok(v) => v,
            Err(err) => {
                ui.label(err);
                return;
            }
        };

        match &self.usage.sub_info_result {
            Ok(maybe_info) => {
                subscription(ui, maybe_info, metrics, Some(uncompressed));

                if maybe_info.is_none() {
                    ui.add_space(25.0);
                    ui.separator();
                    ui.add_space(25.0);

                    ui.heading("Become a Premium user!");
                    ui.add_space(7.0);

                    ui.label("Expand your storage to 30 GB for just $3.00 / month.");
                    ui.add_space(10.0);

                    if ui.button("Upgrade").clicked() {
                        println!("TODO: start the upgrade process!");
                    }
                }
            }
            Err(err) => {
                ui.label(err);
            }
        };
    }

    fn draw_appearance_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Theme Mode:");
        ui.separator();
        ui.add_space(12.0);

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

        // combo box of color aliases
    }

    fn draw_general_tab(&mut self, ui: &mut egui::Ui) {
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

impl super::Modal for SettingsModal {
    const ANCHOR: egui::Align2 = egui::Align2::CENTER_CENTER;
    const Y_OFFSET: f32 = 0.0;

    type Response = ();

    fn title(&self) -> &str {
        "Settings"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        ui.set_max_height(ui.available_size().y - 200.0);
        ui.set_width(500.0);

        StripBuilder::new(ui)
            .size(Size::exact(100.0))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| self.draw_tab_labels(ui));
                strip.cell(|ui| {
                    ui.add_space(12.0);
                    match &self.active_tab {
                        SettingsTab::Account => self.draw_account_tab(ui),
                        SettingsTab::Subscription => self.draw_subscription_tab(ui),
                        SettingsTab::Appearance => self.draw_appearance_tab(ui),
                        SettingsTab::General => self.draw_general_tab(ui),
                    }
                });
            });
    }
}

const EXPORT_DESC: &str = "\
Lockbook encrypts your data with a secret key that remains on your devices. \
Whoever has access to this key has complete knowledge and control of your data.

Do not give this key to anyone. Do not display the QR code if there are cameras around.";
