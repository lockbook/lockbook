use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

use crate::model::{SyncError, Usage};
use crate::theme::Icon;
use crate::widgets::{Button, ProgressBar};

use super::AccountUpdate;

pub struct SyncPanel {
    status: Result<String, String>,
    lock: Arc<Mutex<()>>,
    phase: SyncPhase,
}

impl SyncPanel {
    pub fn new(status: Result<String, String>) -> Self {
        Self { status, lock: Arc::new(Mutex::new(())), phase: SyncPhase::IdleGood }
    }
}

enum SyncPhase {
    IdleGood,
    Syncing,
    IdleError,
}

pub enum SyncUpdate {
    Started,
    Progress(lb::SyncProgress),
    Done(Result<lb::WorkCalculated, SyncError>),
    SetStatus(Result<String, lb::UnexpectedError>),
    SetUsage(Usage),
}

impl super::AccountScreen {
    pub fn process_sync_update(&mut self, ctx: &egui::Context, update: SyncUpdate) {
        match update {
            SyncUpdate::Started => self.sync.phase = SyncPhase::Syncing,
            SyncUpdate::Progress(progress) => {
                let status = match &progress.current_work_unit {
                    lb::ClientWorkUnit::PullMetadata => "Pulling file tree updates".to_string(),
                    lb::ClientWorkUnit::PushMetadata => "Pushing file tree updates".to_string(),
                    lb::ClientWorkUnit::PullDocument(f) => format!("Pulling: {}", f.name),
                    lb::ClientWorkUnit::PushDocument(f) => format!("Pushing: {}", f.name),
                };
                self.sync.status = Ok(status);
            }
            SyncUpdate::Done(result) => match result {
                Ok(_) => {
                    self.sync.status = Ok("just now".to_owned());
                    self.sync.phase = SyncPhase::IdleGood;
                    if let Ok(work) = result {
                        self.refresh_tree_and_workspace(ctx, work);
                        self.suggested.recalc_and_redraw(ctx, &self.core);
                    }
                    self.refresh_sync_status(ctx);

                    let core = self.core.clone();
                    let update_tx = self.update_tx.clone();
                    thread::spawn(move || {
                        update_tx
                            .send(AccountUpdate::FoundPendingShares(
                                !core.get_pending_shares().unwrap().is_empty(),
                            ))
                            .unwrap();
                    });
                }
                Err(err) => {
                    self.sync.phase = SyncPhase::IdleError;
                    match err {
                        SyncError::Minor(msg) => self.sync.status = Err(msg),
                        SyncError::Major(msg) => println!("major sync error: {}", msg), // TODO
                    }
                }
            },
            SyncUpdate::SetStatus(status_result) => self.set_sync_status(status_result),
            SyncUpdate::SetUsage(usage) => self.usage = Ok(usage),
        }
    }

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

                if Button::default()
                    .text("Sync")
                    .padding((6.0, 6.0))
                    .show(ui)
                    .clicked()
                {
                    self.perform_sync(ui.ctx());
                }

                match &self.sync.phase {
                    SyncPhase::IdleGood => (),
                    SyncPhase::Syncing => {
                        ui.spinner();
                    }
                    SyncPhase::IdleError => {
                        Icon::CANCEL.show(ui);
                    }
                };

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

    pub fn refresh_sync_status(&self, ctx: &egui::Context) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let status_result = core.get_last_synced_human_string();
            update_tx
                .send(SyncUpdate::SetStatus(status_result).into())
                .unwrap();
            update_tx
                .send(SyncUpdate::SetUsage(core.get_usage().unwrap().into()).into()) // TODO
                .unwrap();
            ctx.request_repaint();
        });
    }

    pub fn perform_sync(&self, ctx: &egui::Context) {
        if self.sync.lock.try_lock().is_err() {
            return;
        }

        self.save_all_tabs(ctx);

        let sync_lock = self.sync.lock.clone();
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let _lock = sync_lock.lock().unwrap();
            update_tx.send(SyncUpdate::Started.into()).unwrap();
            ctx.request_repaint();

            let closure = {
                let update_tx = update_tx.clone();
                let ctx = ctx.clone();

                move |p: lb::SyncProgress| {
                    update_tx.send(SyncUpdate::Progress(p).into()).unwrap();
                    ctx.request_repaint();
                }
            };

            let result = core.sync(Some(Box::new(closure))).map_err(SyncError::from);
            update_tx.send(SyncUpdate::Done(result).into()).unwrap();
            ctx.request_repaint();
        });
    }

    pub fn perform_final_sync(&self, ctx: &egui::Context) {
        let sync_lock = self.sync.lock.clone();
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let _lock = sync_lock.lock().unwrap();
            if let Err(err) = core.sync(None) {
                eprintln!("error: final sync: {:?}", err);
            }
            update_tx.send(AccountUpdate::FinalSyncAttemptDone).unwrap();
            ctx.request_repaint();
        });
    }
}
