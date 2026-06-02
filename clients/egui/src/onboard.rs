use std::sync::{Arc, RwLock, mpsc};
use std::thread;

use egui::text::LayoutJob;
use egui::{Image, ScrollArea};
use lb::DEFAULT_API_LOCATION;
use lb::blocking::Lb;
use lb::model::errors::LbErr;
use workspace_rs::file_cache::FileCache;
use workspace_rs::theme::palette_v2::ThemeExt;
use workspace_rs::widgets::Button;

use crate::model::AccountPhraseData;
use crate::settings::Settings;

const ONBOARD_CONTROL_ROUNDING: u8 = 3;

pub struct OnboardHandOff {
    pub settings: Arc<RwLock<Settings>>,
    pub core: Lb,
    pub acct_data: FileCache,
    pub is_new_user: bool,
}

enum AccountUpdate {
    Created(Result<FileCache, LbErr>),
    PhraseConfirmation(Result<AccountPhraseData, LbErr>),
    Imported(Option<LbErr>),
    DataLoaded(Result<FileCache, LbErr>),
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
    pub settings: Arc<RwLock<Settings>>,
    pub core: Lb,

    update_tx: mpsc::Sender<AccountUpdate>,
    update_rx: mpsc::Receiver<AccountUpdate>,

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
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Option<OnboardHandOff> {
        let mut resp = None;

        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                AccountUpdate::Created(result) => match result {
                    Ok(acct_data) => {
                        resp = Some(OnboardHandOff {
                            settings: self.settings.clone(),
                            core: self.core.clone(),
                            acct_data,
                            is_new_user: true,
                        });
                    }
                    Err(msg) => {
                        self.router = Router::new(Route::Create, false);
                        self.router.needs_focus = true;
                        self.create_err = Some(msg);
                    }
                },
                AccountUpdate::Imported(maybe_err) => {
                    if let Some(msg) = maybe_err {
                        self.router = Router::new(Route::Import, false);
                        self.import_err = Some(msg);
                    }
                }
                AccountUpdate::DataLoaded(result) => match result {
                    Ok(acct_data) => {
                        resp = Some(OnboardHandOff {
                            settings: self.settings.clone(),
                            core: self.core.clone(),
                            acct_data,
                            is_new_user: false,
                        });
                    }
                    Err(err) => self.import_err = Some(err),
                },
                AccountUpdate::PhraseConfirmation(account_phrase_data) => match account_phrase_data
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

        let left_margin = (500.0_f32).min(ctx.available_rect().width() / 4.5) as i8;
        let top_margin = (100.0_f32).min(ctx.available_rect().height() / 9.0);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().outer_margin(egui::Margin {
                left: left_margin,
                right: 0,
                top: 0,
                bottom: 0,
            }))
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical_centered_justified(|ui| {
                            ui.add_space(top_margin); // apply it here instead of frame, so that scrollbar doesn't get margin but the content does

                            let how_on = ui.ctx().animate_bool_with_time_and_easing(
                                "welcome_route_fade_in".into(),
                                true, // todo: false if first frame
                                1.0,
                                egui::emath::ease_in_ease_out,
                            );
                            ui.set_opacity(how_on);
                            ui.horizontal(|ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                    set_onboard_control_style(ui);

                                    ui.add(self.logo.clone().fit_to_original_size(0.5));
                                    ui.add_space(5.0);
                                    let header_text = match self.router.route {
                                        Route::Create => "Create your account",
                                        Route::Import => "Enter your account key",
                                        Route::Welcome => "Lockbook",
                                        Route::AccountPhraseConfirmation => {
                                            "This is your account key"
                                        }
                                    };
                                    ui.label(egui::RichText::new(header_text).font(
                                        egui::FontId::new(
                                            35.0,
                                            egui::FontFamily::Name(Arc::from("Bold")),
                                        ),
                                    ));

                                    ui.label(self.get_subheader_text());

                                    ui.add_space(70.0);
                                    match self.router.route {
                                        Route::Welcome => {
                                            ui.horizontal(|ui| {
                                                ui.scope(|ui| {
                                                    set_button_style(ui);
                                                    if Button::default()
                                                        .text("Create an account")
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
                                                    let theme = ui.ctx().get_lb_theme();
                                                    let bg =
                                                        theme.bg().get_color(theme.prefs().primary);

                                                    let text_stroke = egui::Stroke {
                                                        color: bg,
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
                                                        .show(ui)
                                                        .clicked()
                                                    {
                                                        self.router.route = Route::Import;
                                                    };
                                                });
                                            });
                                        }
                                        Route::Create => {
                                            ui.horizontal(|ui| {
                                                let resp = show_onboard_text_input(
                                                    ui,
                                                    egui::TextEdit::singleline(&mut self.uname)
                                                        .desired_width(250.0)
                                                        .margin(egui::vec2(8.0, 8.0))
                                                        .hint_text("Pick a username..."),
                                                )
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
                                                        .frame(true)
                                                        .show(ui)
                                                        .clicked()
                                                    {
                                                        self.create_account(ctx);
                                                    };
                                                });
                                            });
                                            self.show_input_err(ui, &self.create_err);
                                        }
                                        Route::Import => {
                                            ui.horizontal(|ui| {
                                                let resp = show_onboard_text_input(
                                                    ui,
                                                    egui::TextEdit::singleline(&mut self.acct_str)
                                                        .desired_width(250.0)
                                                        .margin(egui::vec2(8.0, 8.0))
                                                        .password(true)
                                                        .hint_text("Phrase or compact key"),
                                                )
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
                                                        .frame(true)
                                                        .show(ui)
                                                        .clicked()
                                                    {
                                                        self.import_account(ctx);
                                                    };
                                                });
                                            });
                                            self.show_input_err(ui, &self.import_err);
                                        }
                                        Route::AccountPhraseConfirmation => {
                                            if let Some(account_phrase) = &self.acct_phrase {
                                                let mut col1 = LayoutJob::default();
                                                let mut col2 = LayoutJob::default();
                                                account_phrase.split(' ').enumerate().for_each(
                                                    |(i, word)| {
                                                        let job = if i < 12 {
                                                            &mut col1
                                                        } else {
                                                            &mut col2
                                                        };
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
                                                    .inner_margin(egui::Margin::symmetric(50, 40))
                                                    .corner_radius(3.0)
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
                                                        .show(ui)
                                                        .clicked()
                                                    {
                                                        let core = self.core.clone();
                                                        let update_tx = self.update_tx.clone();
                                                        let ctx = ctx.clone();
                                                        thread::spawn(move || {
                                                            update_tx
                                                                .send(AccountUpdate::Created(
                                                                    FileCache::new(&core),
                                                                ))
                                                                .unwrap();
                                                            ctx.request_repaint();
                                                        });
                                                    }
                                                });
                                            }
                                        }
                                    }

                                    ui.add_space(10.0);
                                    ui.scope(|ui| {
                                        let theme = ui.ctx().get_lb_theme();
                                        let bg = theme.bg().get_color(theme.prefs().primary);

                                        let text_stroke =
                                            egui::Stroke { color: bg, ..Default::default() };
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
                                        };
                                        if let Some((label_text, btn_text, other_route)) =
                                            alternate_route
                                        {
                                            ui.horizontal(|ui| {
                                                ui.set_opacity(0.7);
                                                ui.label(label_text);
                                                if Button::default()
                                                    .text(btn_text)
                                                    .show(ui)
                                                    .clicked()
                                                {
                                                    self.router.route = other_route;
                                                    self.router.needs_focus = true;
                                                }
                                            });
                                        }
                                    });
                                });
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
                r#"Use letters (A-Z), numbers (0-9), and the symbols - _ . @
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
        }
    }

    fn show_input_err(&self, ui: &mut egui::Ui, maybe_err: &Option<LbErr>) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if self.router.is_busy {
                ui.spinner();
                if let Some(s) = &self.import_status {
                    ui.label(s);
                }
            }
            if let Some(err) = &maybe_err {
                ui.label(
                    egui::RichText::new(err.kind.to_string())
                        .color(ui.visuals().error_fg_color)
                        .size(15.0),
                );
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
                .and_then(|_| FileCache::new(&core));

            if let Err(create_account_err) = create_account_result {
                update_tx
                    .send(AccountUpdate::Created(Err(create_account_err)))
                    .unwrap();
                ctx.request_repaint();
                return;
            }

            let account_phrase = core
                .export_account_phrase()
                .map(|res| AccountPhraseData { phrase: res });

            update_tx
                .send(AccountUpdate::PhraseConfirmation(account_phrase))
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
            let api_url =
                std::env::var("API_URL").unwrap_or_else(|_| DEFAULT_API_LOCATION.to_string());

            if let Err(err) = core.import_account(&key, Some(&api_url)) {
                tx.send(AccountUpdate::Imported(Some(err))).unwrap();
                ctx.request_repaint();
                return;
            }

            match core.sync() {
                Ok(_acct) => {
                    tx.send(AccountUpdate::DataLoaded(FileCache::new(&core)))
                        .unwrap();
                }
                Err(err) => {
                    tx.send(AccountUpdate::Imported(Some(err))).unwrap();
                }
            }

            ctx.request_repaint();
        });
    }
}

fn set_button_style(ui: &mut egui::Ui) {
    let theme = ui.ctx().get_lb_theme();
    let bg = theme.bg().get_color(theme.prefs().primary);

    ui.visuals_mut().widgets.inactive.bg_fill = bg;
    ui.visuals_mut().widgets.hovered.bg_fill = bg.gamma_multiply(0.9);
    ui.style_mut().spacing.button_padding += egui::vec2(10.0, 2.0);

    let text_stroke = egui::Stroke { color: theme.neutral_fg(), ..Default::default() };
    ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
    ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
    ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;
}

fn set_onboard_control_style(ui: &mut egui::Ui) {
    let corner_radius = egui::CornerRadius::same(ONBOARD_CONTROL_ROUNDING);

    ui.visuals_mut().widgets.noninteractive.corner_radius = corner_radius;
    ui.visuals_mut().widgets.inactive.corner_radius = corner_radius;
    ui.visuals_mut().widgets.hovered.corner_radius = corner_radius;
    ui.visuals_mut().widgets.active.corner_radius = corner_radius;
    ui.visuals_mut().widgets.open.corner_radius = corner_radius;
}

fn show_onboard_text_input(
    ui: &mut egui::Ui, text_edit: egui::TextEdit<'_>,
) -> egui::text_edit::TextEditOutput {
    let theme = ui.ctx().get_lb_theme();
    let bg = theme.neutral_bg_secondary();
    let stroke = egui::Stroke { width: 1.0, color: theme.neutral() };

    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.bg_stroke = stroke;
        ui.visuals_mut().widgets.hovered.bg_stroke = stroke;
        ui.visuals_mut().widgets.active.bg_stroke = stroke;
        text_edit.frame(true).background_color(bg).show(ui)
    })
    .inner
}

#[derive(Clone, Copy, PartialEq)]
enum Route {
    Create,
    Import,
    Welcome,
    AccountPhraseConfirmation,
}
