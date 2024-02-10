use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;
use workspace_rs::widgets::{Button, ProgressBar};

use super::AccountUpdate;

pub struct SyncPanel {
    status: Result<String, String>,
    lock: Arc<Mutex<()>>,
}

impl SyncPanel {
    pub fn new(status: Result<String, String>) -> Self {
        Self { status, lock: Arc::new(Mutex::new(())) }
    }
}

impl super::AccountScreen {
    pub fn show_sync_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);

        if self.settings.read().unwrap().sidebar_usage {
            match &self.usage {
                Ok(usage) => {
                    egui::Frame::none().inner_margin(6.0).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.columns(2, |uis| {
                                uis[0].horizontal(|ui| {
                                    ui.add_space(2.0);
                                    ui.label(&usage.used);
                                });

                                uis[1].with_layout(
                                    egui::Layout::right_to_left(egui::Align::Min),
                                    |ui| {
                                        ui.add_space(5.0);
                                        ui.label(&usage.available);
                                    },
                                );
                            });
                        });

                        ui.add_space(8.0);

                        ProgressBar::new().percent(usage.percent).show(ui);
                    });
                }
                Err(err) => {
                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(err).color(egui::Color32::RED));
                    });
                }
            }
        } else {
            ui.add_space(10.0);
        }

        let desired_size = egui::vec2(ui.available_size_before_wrap().x, 35.0);
        ui.allocate_ui_with_layout(
            desired_size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(5.0);

                ui.visuals_mut().widgets.inactive.fg_stroke =
                    egui::Stroke { color: ui.visuals().hyperlink_color, ..Default::default() };

                ui.visuals_mut().widgets.hovered.fg_stroke =
                    egui::Stroke { color: ui.visuals().hyperlink_color, ..Default::default() };

                ui.visuals_mut().widgets.active.fg_stroke =
                    egui::Stroke { color: ui.visuals().hyperlink_color, ..Default::default() };

                if !self.workspace.pers_status.syncing
                    && Button::default()
                        .text("Sync")
                        .padding((6.0, 6.0))
                        .show(ui)
                        .clicked()
                {
                    self.workspace.perform_sync();
                }

                if self.workspace.pers_status.syncing {
                    ui.spinner();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    match &self.sync.status {
                        Ok(s) => ui.label(
                            egui::RichText::new(format!("Updated {s}")).color(egui::Color32::GRAY),
                        ),
                        Err(msg) => ui.label(egui::RichText::new(msg).color(egui::Color32::RED)),
                    };
                });
            },
        );

        ui.add_space(20.0);
    }

    pub fn set_sync_status<T: ToString>(&mut self, res: Result<String, T>) {
        self.sync.status = match res {
            Ok(s) => Ok(s),
            Err(v) => Err(v.to_string()),
        };
    }

    pub fn perform_final_sync(&self, ctx: &egui::Context) {
        let sync_lock = self.sync.lock.clone();
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let _lock = sync_lock.lock().unwrap();
            if let Err(err) = core.sync(None) {
                eprintln!("error: final sync: {:?}", err);
            }
            update_tx.send(AccountUpdate::FinalSyncAttemptDone).unwrap();
            ctx.request_repaint();
        });
    }
}
