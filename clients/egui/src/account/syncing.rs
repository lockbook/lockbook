use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Instant, SystemTime};

use eframe::egui;
use lb::Duration;
use workspace_rs::widgets::ProgressBar;

use super::AccountUpdate;

pub struct SyncPanel {
    pub status: Result<String, String>,
    lock: Arc<Mutex<()>>,
    usage_msg_gained_hover: Option<Instant>,
    expanded_usage_msg_rect: egui::Rect,
}

impl SyncPanel {
    pub fn new(status: Result<String, String>) -> Self {
        Self {
            status,
            lock: Arc::new(Mutex::new(())),
            usage_msg_gained_hover: None,
            expanded_usage_msg_rect: egui::Rect::NOTHING,
        }
    }
}

impl super::AccountScreen {
    pub fn show_sync_panel(&mut self, ui: &mut egui::Ui) {
        if self.settings.read().unwrap().sidebar_usage {
            match &self.usage {
                Ok(usage) => {
                    egui::Frame::none().show(ui, |ui| {
                        let is_throttled_hover =
                            if let Some(hover_origin) = self.sync.usage_msg_gained_hover {
                                let throttle_duration = Duration::milliseconds(100);
                                (Instant::now() - hover_origin) > throttle_duration
                            } else {
                                false
                            };

                        let text = if is_throttled_hover {
                            format!("{:.1}% used", usage.percent * 100.)
                        } else {
                            format!("{} out of {} used", usage.used, usage.available)
                        };
                        // )

                        let text: egui::WidgetText = text.into();
                        let text = text.color(ui.visuals().text_color().gamma_multiply(0.8));
                        let galley = text.into_galley(
                            ui,
                            Some(false),
                            ui.available_width(),
                            egui::TextStyle::Small,
                        );

                        let desired_size = egui::vec2(galley.size().x, galley.size().y);
                        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::click());

                        if self.sync.usage_msg_gained_hover.is_none()
                            && !self.sync.expanded_usage_msg_rect.eq(&rect)
                        {
                            self.sync.expanded_usage_msg_rect = rect;
                        }

                        galley.paint_with_visuals(
                            ui.painter(),
                            rect.left_top(),
                            ui.style().interact(&resp),
                        );

                        if self
                            .sync
                            .expanded_usage_msg_rect
                            .expand(5.0)
                            .contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
                        {
                            if self.sync.usage_msg_gained_hover.is_none() {
                                self.sync.usage_msg_gained_hover = Some(Instant::now());
                            }
                        } else {
                            self.sync.usage_msg_gained_hover = None;
                        }

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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // todo: make sure this shows for at least 1 second even if sync finishes before that
                    // if self.workspace.pers_status.syncing {
                    //     ui.spinner();
                    // }
                    // match &self.sync.status {
                    //     Ok(s) => ui.label(
                    //         egui::RichText::new(format!("Updated {s}"))
                    //             .color(ui.visuals().widgets.active.bg_fill)
                    //             .size(15.0),
                    //     ),
                    //     Err(msg) => ui.label(egui::RichText::new(msg).color(egui::Color32::RED)),
                    // };
                });
            },
        );
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
