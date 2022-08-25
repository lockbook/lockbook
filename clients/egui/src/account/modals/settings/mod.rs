mod account_tab;
mod appearance_tab;
mod general_tab;
mod usage_tab;

use std::sync::{Arc, RwLock};

use eframe::egui;
use egui_extras::{Size, StripBuilder};

use crate::settings::Settings;

use self::account_tab::*;
use self::usage_tab::*;

enum SettingsTab {
    Account,
    Usage,
    Appearance,
    General,
}

pub struct SettingsModal {
    core: Arc<lb::Core>,
    settings: Arc<RwLock<Settings>>,
    account: AccountSettings,
    usage: UsageSettings,
    active_tab: SettingsTab,
}

pub enum SettingsResponse {
    SuccessfullyUpgraded,
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
            core: core.clone(),
            settings: s.clone(),
            account: AccountSettings { export_result },
            usage: UsageSettings {
                sub_info_result,
                metrics_result,
                uncompressed_result,
                upgrading: None,
            },
            active_tab: SettingsTab::Account,
        }))
    }

    fn draw_tab_labels(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            if ui.button("Account").clicked() {
                self.active_tab = SettingsTab::Account;
            }
            if ui.button("Usage").clicked() {
                self.active_tab = SettingsTab::Usage;
            }
            if ui.button("Appearance").clicked() {
                self.active_tab = SettingsTab::Appearance;
            }
            if ui.button("General").clicked() {
                self.active_tab = SettingsTab::General;
            }
        });
    }
}

impl super::Modal for SettingsModal {
    const ANCHOR: egui::Align2 = egui::Align2::CENTER_CENTER;
    const Y_OFFSET: f32 = 0.0;

    type Response = Option<SettingsResponse>;

    fn title(&self) -> &str {
        "Settings"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        let mut resp = None;

        ui.set_max_height(ui.available_size().y - 300.0);
        ui.set_width(520.0);

        StripBuilder::new(ui)
            .size(Size::exact(100.0))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| self.draw_tab_labels(ui));
                strip.cell(|ui| {
                    ui.add_space(12.0);
                    match &self.active_tab {
                        SettingsTab::Account => self.show_account_tab(ui),
                        SettingsTab::Usage => {
                            resp = self.show_usage_tab(ui);
                        }
                        SettingsTab::Appearance => self.show_appearance_tab(ui),
                        SettingsTab::General => self.show_general_tab(ui),
                    }
                });
            });

        resp
    }
}
