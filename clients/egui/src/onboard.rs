use std::sync::{mpsc, Arc, RwLock};
use std::thread;

use eframe::egui;
use egui_extras::RetainedImage;

use crate::model::{AccountScreenInitData, SyncError};
use crate::settings::Settings;
use crate::widgets::ButtonGroup;

pub struct OnboardHandOff {
    pub settings: Arc<RwLock<Settings>>,
    pub core: lb::Core,
    pub acct_data: AccountScreenInitData,
}

enum Update {
    AccountCreated(Result<AccountScreenInitData, String>),
    AccountImported(Option<String>),
    ImportSyncProgress(lb::SyncProgress),
    ImportSyncDone(Option<SyncError>),
    AccountDataLoaded(Result<AccountScreenInitData, String>),
}

enum State {
    Idle(Route),
    Busy(Route),
}

pub struct OnboardScreen {
    settings: Arc<RwLock<Settings>>,
    core: lb::Core,

    update_tx: mpsc::Sender<Update>,
    update_rx: mpsc::Receiver<Update>,

    state: State,
    logo: RetainedImage,
    route_needs_focus: Option<Route>,

    uname: String,
    create_err: Option<String>,

    acct_str: String,
    import_err: Option<String>,
    import_status: Option<String>,
}

impl OnboardScreen {
    pub fn new(settings: Arc<RwLock<Settings>>, core: lb::Core) -> Self {
        let (update_tx, update_rx) = mpsc::channel();

        Self {
            settings,
            core,
            update_tx,
            update_rx,
            state: State::Idle(Route::Create),
            logo: RetainedImage::from_image_bytes("onboard-logo", LOGO).unwrap(),
            route_needs_focus: Some(Route::Create),
            uname: String::new(),
            create_err: None,
            acct_str: String::new(),
            import_err: None,
            import_status: None,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Option<OnboardHandOff> {
        let mut resp = None;

        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                Update::AccountCreated(result) => match result {
                    Ok(acct_data) => {
                        resp = Some(OnboardHandOff {
                            settings: self.settings.clone(),
                            core: self.core.clone(),
                            acct_data,
                        });
                    }
                    Err(msg) => {
                        self.state = State::Idle(Route::Create);
                        self.create_err = Some(msg);
                    }
                },
                Update::AccountImported(maybe_err) => {
                    if let Some(msg) = maybe_err {
                        self.state = State::Idle(Route::Import);
                        self.import_err = Some(msg);
                    }
                }
                Update::ImportSyncProgress(sp) => {
                    self.import_status = Some(match &sp.current_work_unit {
                        lb::ClientWorkUnit::PullMetadata => "Pulling file tree updates".to_string(),
                        lb::ClientWorkUnit::PushMetadata => "Pushing file tree updates".to_string(),
                        lb::ClientWorkUnit::PullDocument(f) => format!("Pulling: {}", f.name),
                        lb::ClientWorkUnit::PushDocument(f) => format!("Pushing: {}", f.name),
                    });
                }
                Update::ImportSyncDone(maybe_err) => {
                    if let Some(err) = maybe_err {
                        match err {
                            SyncError::Major(msg) => self.import_err = Some(msg),
                            SyncError::Minor(msg) => self.import_err = Some(msg),
                            SyncError::UsageIsOverDataCap => {
                                self.import_err =
                                    Some("Usage is over data dap. You need to Upgrade".to_string())
                            }
                        }
                    } else {
                        self.import_status = Some("Loading account data...".to_string());
                    }
                }
                Update::AccountDataLoaded(result) => match result {
                    Ok(acct_data) => {
                        resp = Some(OnboardHandOff {
                            settings: self.settings.clone(),
                            core: self.core.clone(),
                            acct_data,
                        });
                    }
                    Err(err) => self.import_err = Some(err),
                },
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(400.0);

                ui.add_space(40.0);
                self.logo.show_scaled(ui, 1.0);

                ui.add_space(50.0);
                ui.label(egui::RichText::new("Lockbook").size(48.0));

                ui.add_space(25.0);
                ui.separator();
                ui.add_space(30.0);

                match &mut self.state {
                    State::Idle(ref mut route) => {
                        ui.horizontal(|ui| {
                            if let Some(route_clicked) = ButtonGroup::toggle(*route)
                                .btn(Route::Create, "Create Account")
                                .btn(Route::Import, "Import Account")
                                .hcenter()
                                .show(ui)
                            {
                                *route = route_clicked;
                                self.route_needs_focus = Some(route_clicked);
                            }
                        });

                        ui.add_space(30.0);

                        match route {
                            Route::Create => {
                                let resp = egui::TextEdit::singleline(&mut self.uname)
                                    .margin(egui::vec2(8.0, 8.0))
                                    .hint_text("Pick a username...")
                                    .show(ui)
                                    .response;

                                if resp.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    self.create_account(ctx);
                                }

                                if self.route_needs_focus == Some(Route::Create) {
                                    resp.request_focus();
                                }

                                if let Some(err) = &self.create_err {
                                    ui.label(err);
                                }
                            }
                            Route::Import => {
                                let resp = egui::TextEdit::singleline(&mut self.acct_str)
                                    .margin(egui::vec2(8.0, 8.0))
                                    .hint_text("Account secret...")
                                    .password(true)
                                    .show(ui)
                                    .response;

                                if resp.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    self.import_account(ctx);
                                }

                                if self.route_needs_focus == Some(Route::Import) {
                                    resp.request_focus();
                                }

                                if let Some(err) = &self.import_err {
                                    ui.label(err);
                                }
                            }
                        };

                        self.route_needs_focus = None;
                    }
                    State::Busy(route) => match route {
                        Route::Create => {
                            ui.spinner();
                            ui.heading("Creating account...");
                        }
                        Route::Import => {
                            ui.spinner();
                            ui.heading("Importing account...");
                            if let Some(s) = &self.import_status {
                                ui.add_space(8.0);
                                ui.label(s);
                            }
                        }
                    },
                };
            });
        });

        resp
    }

    fn create_account(&mut self, ctx: &egui::Context) {
        self.state = State::Busy(Route::Create);

        let core = self.core.clone();
        let uname = self.uname.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let api_url =
                std::env::var("API_URL").unwrap_or_else(|_| lb::DEFAULT_API_LOCATION.to_string());

            let result = core
                .create_account(&uname, &api_url, true)
                .map_err(|err| format!("{:?}", err))
                .and_then(|_| load_account_data(&core));

            update_tx.send(Update::AccountCreated(result)).unwrap();
            ctx.request_repaint();
        });
    }

    fn import_account(&mut self, ctx: &egui::Context) {
        self.state = State::Busy(Route::Import);

        let core = self.core.clone();
        let key = self.acct_str.clone();
        let tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            if let Err(err) = core
                .import_account(&key)
                .map_err(|err| format!("{:?}", err))
            {
                tx.send(Update::AccountImported(Some(err))).unwrap();
                ctx.request_repaint();
                return;
            }

            let closure = {
                let ctx = ctx.clone();
                let tx = tx.clone();

                move |msg| {
                    tx.send(Update::ImportSyncProgress(msg)).unwrap();
                    ctx.request_repaint();
                }
            };

            match core.sync(Some(Box::new(closure))).map_err(SyncError::from) {
                Ok(_acct) => {
                    tx.send(Update::ImportSyncDone(None)).unwrap();
                    tx.send(Update::AccountDataLoaded(load_account_data(&core)))
                        .unwrap();
                }
                Err(err) => {
                    tx.send(Update::ImportSyncDone(Some(err))).unwrap();
                }
            }

            ctx.request_repaint();
        });
    }
}

fn load_account_data(core: &lb::Core) -> Result<AccountScreenInitData, String> {
    let files = match core.list_metadatas() {
        Ok(files) => files,
        Err(err) => return Err(format!("{:?}", err)), // TODO
    };

    let usage = match core.get_usage() {
        Ok(metrics) => Ok(metrics.into()),
        Err(err) => return Err(format!("{:?}", err)), // TODO
    };

    let has_pending_shares = match core.get_pending_shares() {
        Ok(files) => !files.is_empty(),
        Err(err) => {
            eprintln!("{:?}", err);
            false
        }
    };

    let sync_status = core
        .get_last_synced_human_string()
        .map_err(|err| format!("{:?}", err));

    Ok(AccountScreenInitData { sync_status, files, usage, has_pending_shares })
}

#[derive(Clone, Copy, PartialEq)]
enum Route {
    Create,
    Import,
}

const LOGO: &[u8] = include_bytes!("../lockbook-backdrop.png");
