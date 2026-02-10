use std::sync::{Arc, RwLock, mpsc};
use std::thread;

use egui::text::LayoutJob;
use egui::{Image, ScrollArea};
use lb::DEFAULT_API_LOCATION;
use lb::blocking::Lb;
use lb::model::errors::LbErr;
use lb::model::file::File;
use lb::service::sync::SyncProgress;
use workspace_rs::widgets::Button;

use crate::model::AccountPhraseData;
use crate::settings::Settings;

pub struct OnboardHandOff {
    pub settings: Arc<RwLock<Settings>>,
    pub core: Lb,
    pub acct_data: Vec<File>,
}

enum Update {
    AccountCreated(Result<Vec<File>, LbErr>),
    AccountPhraseConfirmation(Result<AccountPhraseData, LbErr>),
    AccountImported(Option<LbErr>),
    ImportSyncProgress(SyncProgress),
    ImportSyncDone(Option<LbErr>),
    AccountDataLoaded(Result<Vec<File>, LbErr>),
}

struct Router {
    route: Route,
    is_busy: bool,
    needs_focus: bool,
}

impl Router {
    fn new(route: Route, is_busy: bool) -> Self {
        Self { route, is_busy, needs_focus: false }
    }
}

pub struct OnboardScreen {
    settings: Arc<RwLock<Settings>>,
    pub core: Lb,

    update_tx: mpsc::Sender<Update>,
    update_rx: mpsc::Receiver<Update>,

    router: Router,
    logo: Image<'static>,

    uname: String,
    create_err: Option<LbErr>,

    acct_str: String,
    acct_phrase: Option<String>,
    acct_phrase_stored: bool,
    confirm_phrase_err: Option<LbErr>,

    import_err: Option<LbErr>,
    import_status: Option<String>,

    text_rect: Option<egui::Rect>,
}

impl OnboardScreen {
    pub fn new(settings: Arc<RwLock<Settings>>, core: Lb) -> Self {
        let (update_tx, update_rx) = mpsc::channel();

        Self {
            settings,
            core,
            update_tx,
            update_rx,
            router: Router::new(Route::Welcome, false),
            logo: Image::new(egui::include_image!("../../../libs/content/workspace/logo.png")),
            uname: String::new(),
            create_err: None,
            acct_str: String::new(),
            acct_phrase: None,
            acct_phrase_stored: false,
            confirm_phrase_err: None,
            import_err: None,
            import_status: None,
            text_rect: None,
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
                        self.router = Router::new(Route::Create, false);
                        self.router.needs_focus = true;
                        self.create_err = Some(msg);
                    }
                },
                Update::AccountImported(maybe_err) => {
                    if let Some(msg) = maybe_err {
                        self.router = Router::new(Route::Import, false);
                        self.import_err = Some(msg);
                    }
                }
                Update::ImportSyncProgress(sp) => {
                    self.import_status = Some(sp.to_string());
                    self.router = Router::new(Route::SyncProgress, true)
                }
                Update::ImportSyncDone(maybe_err) => {
                    if let Some(err) = maybe_err {
                        self.import_err = Some(err);
                        self.router.needs_focus = true;
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
                Update::AccountPhraseConfirmation(account_phrase_data) => match account_phrase_data
                {
                    Ok(phrase_data) => {
                        self.router = Router::new(Route::AccountPhraseConfirmation, false);
                        self.acct_phrase = Some(phrase_data.phrase)
                    }
                    Err(err) => {
                        self.confirm_phrase_err = Some(err);
                    }
                },
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    let how_on = ui.ctx().animate_bool_with_time_and_easing(
                        "welcome_route_fade_in".into(),
                        ui.ctx().frame_nr() > 1,
                        1.0,
                        egui::emath::ease_in_ease_out,
                    );
                    ui.set_opacity(how_on);
                    ui.horizontal(|ui| {
                        let left_margin = if let Some(rect) = self.text_rect {
                            if rect.width() > ui.available_width() {
                                0.0
                            } else {
                                ui.available_width() / 5.0
                            }
                        } else {
                            100.0
                        };

                        ui.add_space(left_margin);
                        let rect = ui
                            .with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                ui.add_space(100.0);
                                ui.add(self.logo.clone().fit_to_original_size(0.5));
                                ui.add_space(5.0);
                                let header_text = match self.router.route {
                                    Route::Create => "Create your account",
                                    Route::Import => "Enter your account key",
                                    Route::Welcome => "Lockbook",
                                    Route::AccountPhraseConfirmation => "This is your account key",
                                    Route::SyncProgress => "Importing data",
                                };
                                ui.label(egui::RichText::new(header_text).font(egui::FontId::new(
                                    35.0,
                                    egui::FontFamily::Name(Arc::from("Bold")),
                                )));

                                ui.label(self.get_subheader_text());

                                ui.add_space(70.0);
                                match self.router.route {
                                    Route::Welcome => {
                                        ui.horizontal(|ui| {
                                            ui.scope(|ui| {
                                                set_button_style(ui);
                                                if Button::default()
                                                    .text("Create an account")
                                                    .rounding(egui::Rounding::same(3.0))
                                                    .frame(true)
                                                    .show(ui)
                                                    .clicked()
                                                {
                                                    self.router.route = Route::Create;
                                                    self.router.needs_focus = true;
                                                };
                                            });
                                            ui.add_space(5.0);
                                            ui.scope(|ui| {
                                                let text_stroke = egui::Stroke {
                                                    color: ui.visuals().widgets.active.bg_fill,
                                                    ..Default::default()
                                                };
                                                ui.visuals_mut().widgets.inactive.fg_stroke =
                                                    text_stroke;
                                                ui.visuals_mut().widgets.active.fg_stroke =
                                                    text_stroke;
                                                ui.visuals_mut().widgets.hovered.fg_stroke =
                                                    text_stroke;
                                                if Button::default()
                                                    .text("I have an account")
                                                    .rounding(egui::Rounding::same(3.0))
                                                    .show(ui)
                                                    .clicked()
                                                {
                                                    self.router.route = Route::Import;
                                                };
                                            });
                                        });
                                    }
                                    Route::Create => {
                                        let input_resp = ui
                                            .horizontal(|ui| {
                                                let resp =
                                                    egui::TextEdit::singleline(&mut self.uname)
                                                        .desired_width(250.0)
                                                        .margin(egui::vec2(8.0, 8.0))
                                                        .hint_text("Pick a username...")
                                                        .show(ui)
                                                        .response;
                                                if resp.lost_focus()
                                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                                {
                                                    self.create_account(ctx);
                                                }
                                                if resp.changed() {
                                                    self.create_err = None;
                                                }
                                                if self.router.needs_focus {
                                                    resp.request_focus();
                                                    self.router.needs_focus = false;
                                                }
                                                ui.scope(|ui| {
                                                    set_button_style(ui);
                                                    if Button::default()
                                                        .text("Create account")
                                                        .rounding(egui::Rounding::same(3.0))
                                                        .frame(true)
                                                        .show(ui)
                                                        .clicked()
                                                    {
                                                        self.create_account(ctx);
                                                    };
                                                });
                                            })
                                            .response;
                                        self.show_input_err(ui, input_resp, &self.create_err);
                                    }
                                    Route::Import => {
                                        let input_resp = ui
                                            .horizontal(|ui| {
                                                let resp =
                                                    egui::TextEdit::singleline(&mut self.acct_str)
                                                        .desired_width(250.0)
                                                        .margin(egui::vec2(8.0, 8.0))
                                                        .margin(egui::vec2(8.0, 8.0))
                                                        .hint_text("Account secret...")
                                                        .margin(egui::vec2(8.0, 8.0))
                                                        .hint_text("Account secret...")
                                                        .password(true)
                                                        .hint_text("Phrase or compact key")
                                                        .show(ui)
                                                        .response;
                                                if resp.lost_focus()
                                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                                {
                                                    self.import_account(ctx);
                                                }
                                                if self.router.needs_focus {
                                                    resp.request_focus();
                                                    self.router.needs_focus = false;
                                                }
                                                if resp.changed() {
                                                    self.import_err = None;
                                                }
                                                ui.scope(|ui| {
                                                    set_button_style(ui);
                                                    if Button::default()
                                                        .text("Import account")
                                                        .rounding(egui::Rounding::same(3.0))
                                                        .frame(true)
                                                        .show(ui)
                                                        .clicked()
                                                    {
                                                        self.import_account(ctx);
                                                    };
                                                });
                                            })
                                            .response;
                                        self.show_input_err(ui, input_resp, &self.import_err);
                                    }
                                    Route::AccountPhraseConfirmation => {
                                        if let Some(account_phrase) = &self.acct_phrase {
                                            let mut col1 = LayoutJob::default();
                                            let mut col2 = LayoutJob::default();
                                            account_phrase.split(' ').enumerate().for_each(
                                                |(i, word)| {
                                                    let job =
                                                        if i < 12 { &mut col1 } else { &mut col2 };
                                                    job.append(
                                                        &format!("{}. ", i + 1),
                                                        0.0,
                                                        egui::TextFormat {
                                                            font_id: egui::FontId::new(
                                                                14.0,
                                                                egui::FontFamily::Monospace,
                                                            ),
                                                            color: ui
                                                                .visuals()
                                                                .widgets
                                                                .active
                                                                .bg_fill,
                                                            ..Default::default()
                                                        },
                                                    );
                                                    job.append(
                                                        &format!("{word}\n"),
                                                        0.0,
                                                        egui::TextFormat {
                                                            font_id: egui::FontId::new(
                                                                14.0,
                                                                egui::FontFamily::Monospace,
                                                            ),
                                                            color: ui.visuals().text_color(),
                                                            ..Default::default()
                                                        },
                                                    );
                                                },
                                            );

                                            egui::Frame::default()
                                                .fill(ui.visuals().code_bg_color)
                                                .inner_margin(egui::Margin::symmetric(50.0, 40.0))
                                                .rounding(3.0)
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(col1);
                                                        ui.add_space(30.0);
                                                        ui.label(col2);
                                                    });
                                                });
                                            ui.checkbox(
                                                &mut self.acct_phrase_stored,
                                                "I've stored my account key in a safe place.",
                                            );
                                            ui.add_space(20.0);
                                            set_button_style(ui);
                                            ui.add_enabled_ui(self.acct_phrase_stored, |ui| {
                                                if Button::default()
                                                    .text("Create account")
                                                    .frame(true)
                                                    .rounding(egui::Rounding::same(3.0))
                                                    .show(ui)
                                                    .clicked()
                                                {
                                                    let core = self.core.clone();
                                                    let update_tx = self.update_tx.clone();
                                                    let ctx = ctx.clone();
                                                    thread::spawn(move || {
                                                        let result = load_account_data(&core);
                                                        update_tx
                                                            .send(Update::AccountCreated(result))
                                                            .unwrap();
                                                        ctx.request_repaint();
                                                    });
                                                }
                                            });
                                        }
                                    }
                                    Route::SyncProgress => {
                                        if let Some(s) = &self.import_status {
                                            ui.label(s);
                                        }
                                    }
                                }

                                ui.add_space(200.0);
                                ui.scope(|ui| {
                                    let text_stroke = egui::Stroke {
                                        color: ui.visuals().widgets.active.bg_fill,
                                        ..Default::default()
                                    };
                                    ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
                                    ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
                                    ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;
                                    ui.style_mut().text_styles.insert(
                                        egui::TextStyle::Body,
                                        egui::FontId::proportional(13.0),
                                    );
                                    ui.style_mut().spacing.button_padding = egui::Vec2::ZERO;

                                    let alternate_route = match self.router.route {
                                        Route::Create => Some((
                                            "Already have an account?",
                                            "Import your account",
                                            Route::Import,
                                        )),
                                        Route::Import => Some((
                                            "Don't have an account?",
                                            "Create an account",
                                            Route::Create,
                                        )),
                                        Route::Welcome => None,
                                        Route::AccountPhraseConfirmation => None,
                                        Route::SyncProgress => None,
                                    };
                                    if let Some((label_text, btn_text, other_route)) =
                                        alternate_route
                                    {
                                        ui.horizontal(|ui| {
                                            ui.set_opacity(0.7);
                                            ui.label(label_text);
                                            if Button::default().text(btn_text).show(ui).clicked() {
                                                self.router.route = other_route;
                                                self.router.needs_focus = true;
                                            }
                                        });
                                    }
                                });
                            })
                            .response
                            .rect;

                        self.text_rect = Some(rect);
                    });
                    ui.add_space(30.0);
                });
            });
        });
        resp
    }

    fn get_subheader_text(&mut self) -> &str {
        match self.router.route {
            Route::Create => {
                r#"Use letters (A-Z) and numbers (0-9). Special characters aren't allowed.
You can't change your username later."#
            }
            Route::Import => {
                r#"Enter your phrase or private key.
If you enter a phrase, please separate each word by a space or comma."#
            }
            Route::Welcome => {
                r#"The private note-taking platform.
The perfect place to record, sync, and share your thoughts."#
            }
            Route::AccountPhraseConfirmation => {
                r#"It proves you're you, and it is a secret. If you lose it, you can't recover your account.
You can view your key again in the settings."#
            }
            Route::SyncProgress => "",
        }
    }

    fn show_input_err(&self, ui: &mut egui::Ui, resp: egui::Response, maybe_err: &Option<LbErr>) {
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0.0, egui::Color32::DEBUG_COLOR);
        let resp_bottom_left = resp.rect.min + egui::vec2(0.0, resp.rect.height() + 20.0);
        let error_rect =
            egui::Rect::from_min_size(resp_bottom_left, ui.available_size_before_wrap());

        let mut ui = ui.child_ui(
            ui.available_rect_before_wrap(),
            egui::Layout::top_down(egui::Align::LEFT),
            None,
        );
        ui.allocate_ui_at_rect(error_rect, |ui| {
            if let Some(err) = &maybe_err {
                egui::ScrollArea::vertical()
                    .max_height(100.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(err.kind.to_string())
                                .color(ui.visuals().error_fg_color)
                                .size(15.0),
                        );
                    });
            }
            if self.router.is_busy {
                ui.spinner();
                if let Some(s) = &self.import_status {
                    ui.label(s);
                }
            }
        });
    }

    fn create_account(&mut self, ctx: &egui::Context) {
        self.router = Router::new(Route::Create, true);

        let core = self.core.clone();
        let uname = self.uname.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let api_url =
                std::env::var("API_URL").unwrap_or_else(|_| DEFAULT_API_LOCATION.to_string());

            let create_account_result = core
                .create_account(&uname, &api_url, true)
                .and_then(|_| load_account_data(&core));

            if let Err(create_account_err) = create_account_result {
                update_tx
                    .send(Update::AccountCreated(Err(create_account_err)))
                    .unwrap();
                ctx.request_repaint();
                return;
            }

            let account_phrase = core
                .export_account_phrase()
                .map(|res| AccountPhraseData { phrase: res });

            update_tx
                .send(Update::AccountPhraseConfirmation(account_phrase))
                .unwrap();
            ctx.request_repaint();
        });
    }

    fn import_account(&mut self, ctx: &egui::Context) {
        self.router = Router::new(Route::Import, true);

        let core = self.core.clone();
        let key = self.acct_str.clone();
        let tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            if let Err(err) = core.import_account(&key, None) {
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

            match core.sync(Some(Box::new(closure))) {
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

fn set_button_style(ui: &mut egui::Ui) {
    ui.visuals_mut().widgets.inactive.bg_fill = ui.visuals().widgets.active.bg_fill;
    ui.visuals_mut().widgets.hovered.bg_fill =
        ui.visuals().widgets.active.bg_fill.gamma_multiply(0.9);
    ui.style_mut().spacing.button_padding += egui::vec2(10.0, 2.0);

    let text_stroke = egui::Stroke { color: ui.visuals().extreme_bg_color, ..Default::default() };
    ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
    ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
    ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;
}

fn load_account_data(core: &Lb) -> Result<Vec<File>, LbErr> {
    let files = core.list_metadatas()?;

    Ok(files)
}

#[derive(Clone, Copy, PartialEq)]
enum Route {
    Create,
    Import,
    Welcome,
    AccountPhraseConfirmation,
    SyncProgress,
}
