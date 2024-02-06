mod account_tab;
mod appearance_tab;
mod general_tab;
mod usage_tab;

use std::sync::{mpsc, Arc, RwLock};

use eframe::egui;
use egui_extras::{Size, StripBuilder};
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::{separator, ToolBarVisibility};

use crate::settings::Settings;

use self::account_tab::*;
use self::usage_tab::*;

#[derive(PartialEq)]
enum SettingsTab {
    Account,
    Usage,
    Appearance,
    General,
}

pub struct SettingsModal {
    core: lb::Core,
    settings: Arc<RwLock<Settings>>,
    account: AccountSettings,
    usage: UsageSettings,
    active_tab: SettingsTab,
    version: String,
}

pub enum SettingsResponse {
    SuccessfullyUpgraded,
    ToggleToolbarVisibility(ToolBarVisibility),
}

impl SettingsModal {
    pub fn new(core: &lb::Core, s: &Arc<RwLock<Settings>>) -> Self {
        let export_result = core.export_account().map_err(|err| format!("{:?}", err)); // TODO

        let (info_tx, info_rx) = mpsc::channel();

        std::thread::spawn({
            let core = core.clone();
            let info_tx = info_tx.clone();

            move || {
                let sub_info_result = core
                    .get_subscription_info()
                    .map_err(|err| format!("{:?}", err)); // TODO

                let metrics_result = core.get_usage().map_err(|err| format!("{:?}", err)); // TODO
                let uncompressed_result = core
                    .get_uncompressed_usage()
                    .map_err(|err| format!("{:?}", err)); // TODO

                let usage_info =
                    UsageSettingsInfo { sub_info_result, metrics_result, uncompressed_result };

                info_tx.send(usage_info).unwrap();
            }
        });

        Self {
            core: core.clone(),
            settings: s.clone(),
            account: AccountSettings::new(export_result),
            usage: UsageSettings { info: None, info_tx, info_rx, upgrading: None },
            active_tab: SettingsTab::Account,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn show_tab_labels(&mut self, ui: &mut egui::Ui) {
        ui.add_space(1.0);
        egui::Frame::none()
            .fill(ui.visuals().faint_bg_color)
            .rounding(egui::Rounding {
                sw: ui.style().visuals.window_rounding.sw,
                ..Default::default()
            })
            .show(ui, |ui| {
                ui.set_min_height(ui.available_size_before_wrap().y);
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                self.tab_label(ui, SettingsTab::Account, Icon::ACCOUNT, "Account");
                self.tab_label(ui, SettingsTab::Usage, Icon::SAVE, "Usage");
                self.tab_label(ui, SettingsTab::Appearance, Icon::SPARKLE, "Appearance");
                self.tab_label(ui, SettingsTab::General, Icon::SETTINGS, "General");
            });
    }

    fn tab_label(&mut self, ui: &mut egui::Ui, tab: SettingsTab, icon: Icon, text: &str) {
        const PADDING: f32 = 15.0;
        const SPACING: f32 = 10.0;

        let text_height = ui.text_style_height(&egui::TextStyle::Body);
        let height = 24.0 + text_height + SPACING + PADDING * 2.0;

        let response = ui
            .scope(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.set_height(height);

                StripBuilder::new(ui)
                    .size(Size::remainder()) // Main tab content.
                    .size(Size::exact(4.0)) // Active bar indicator.
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(PADDING);
                                icon.size(24.0).show(ui);
                                ui.add_space(SPACING);
                                ui.label(text);
                                ui.add_space(PADDING);
                            });
                        });
                        strip.cell(|ui| {
                            let available_space = ui.available_size_before_wrap();
                            let size = egui::vec2(4.0, available_space.y);

                            let (rect, response) = ui.allocate_at_least(size, egui::Sense::hover());

                            if ui.is_rect_visible(response.rect) {
                                let color = if self.active_tab == tab {
                                    ui.visuals().widgets.active.bg_fill
                                } else {
                                    ui.visuals().widgets.noninteractive.bg_stroke.color
                                };

                                let stroke = egui::Stroke::new(4.0, color);

                                ui.painter().vline(rect.center().x, rect.y_range(), stroke);
                            }
                        });
                    });
            })
            .response;

        let response = ui.interact(
            response.rect,
            egui::Id::from(format!("tab_label_{}", text)),
            egui::Sense::click(),
        );

        if response.clicked() {
            self.active_tab = tab;
        }
        if response.hovered() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }

        separator(ui);
    }

    fn show_version(&self, ui: &mut egui::Ui) -> egui::InnerResponse<()> {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.add_space(15.0);
            ui.horizontal(|ui| {
                ui.add_space(15.0);
                ui.label(
                    egui::RichText::from(format!("Version: {}", &self.version))
                        .color(egui::Color32::GRAY),
                );
            });
        })
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

        ui.set_max_height(ui.available_size().y - 400.0);
        ui.set_width(520.0);

        StripBuilder::new(ui)
            .size(Size::exact(115.0))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| self.show_tab_labels(ui));
                strip.cell(|ui| {
                    ui.add_space(12.0);
                    match &self.active_tab {
                        SettingsTab::Account => self.show_account_tab(ui),
                        SettingsTab::Usage => {
                            resp = self.show_usage_tab(ui);
                        }
                        SettingsTab::Appearance => {
                            resp = self.show_appearance_tab(ui);
                        }
                        SettingsTab::General => self.show_general_tab(ui),
                    }
                    self.show_version(ui);
                });
            });

        resp
    }
}
