use std::sync::{Arc, RwLock, mpsc};

use lb::blocking::Lb;
use lb::model::core_config::Config;
use lb::model::errors::LbErrKind;
use lb::model::file::File;

use crate::settings::Settings;

pub struct SplashHandOff {
    pub settings: Arc<RwLock<Settings>>,
    pub core: Lb,
    pub maybe_files: Option<Vec<File>>,
}

#[allow(clippy::large_enum_variant)]
enum SplashUpdate {
    Status(String),
    Error(String),
    Done((Lb, Option<Vec<File>>)),
}

pub struct SplashScreen {
    settings: Arc<RwLock<Settings>>,

    update_tx: mpsc::Sender<SplashUpdate>,
    update_rx: mpsc::Receiver<SplashUpdate>,

    maybe_error: Option<String>,
    status: Option<String>,
}

impl SplashScreen {
    pub fn new(settings: Arc<RwLock<Settings>>, maybe_error: Option<String>) -> Self {
        let (update_tx, update_rx) = mpsc::channel();

        Self { settings, update_tx, update_rx, maybe_error, status: None }
    }

    pub fn start_loading_core(&self, ctx: &egui::Context) {
        if self.maybe_error.is_some() {
            return;
        }

        let ctx = ctx.clone();
        let tx = self.update_tx.clone();

        std::thread::spawn(move || {
            let cfg = Config::ui_config("egui");

            tx.send(SplashUpdate::Status("Loading core...".to_string()))
                .unwrap();

            let core = match Lb::init(cfg) {
                Ok(core) => core,
                Err(err) => {
                    tx.send(SplashUpdate::Error(format!("{err:?}"))).unwrap();
                    ctx.request_repaint();
                    return;
                }
            };
            let is_signed_in = match is_signed_in(&core) {
                Ok(is_signed_in) => is_signed_in,
                Err(err) => {
                    tx.send(SplashUpdate::Error(format!("{err:?}"))).unwrap();
                    ctx.request_repaint();
                    return;
                }
            };

            if is_signed_in {
                tx.send(SplashUpdate::Status("Loading files...".to_string()))
                    .unwrap();

                let files = match core.list_metadatas() {
                    Ok(files) => files,
                    Err(err) => {
                        tx.send(SplashUpdate::Error(format!("{err:?}"))).unwrap();
                        ctx.request_repaint();
                        return;
                    }
                };

                tx.send(SplashUpdate::Done((core, Some(files)))).unwrap();
            } else {
                tx.send(SplashUpdate::Done((core, None))).unwrap();
            }

            ctx.request_repaint();
        });
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Option<SplashHandOff> {
        let mut resp = None;

        // Process any pending updates.
        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                SplashUpdate::Status(msg) => self.status = Some(msg),
                SplashUpdate::Error(msg) => self.maybe_error = Some(msg),
                SplashUpdate::Done((core, maybe_acct_data)) => {
                    self.status = Some("Done.".to_string());
                    resp = Some(SplashHandOff {
                        settings: self.settings.clone(),
                        core,
                        maybe_files: maybe_acct_data,
                    });
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                if let Some(msg) = &self.maybe_error {
                    ui.label(egui::RichText::new(msg).color(egui::Color32::RED));
                } else if let Some(status) = &self.status {
                    ui.heading(status);
                } else {
                    ui.spinner();
                }
            });
        });

        resp
    }
}

fn is_signed_in(core: &Lb) -> Result<bool, String> {
    match core.get_account() {
        Ok(_acct) => Ok(true),
        Err(err) => match err.kind {
            LbErrKind::AccountNonexistent => Ok(false),
            _ => Err(format!("{err:?}")), // todo(steve): display
        },
    }
}
