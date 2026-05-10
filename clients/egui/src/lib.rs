#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod model;
mod onboard;
mod settings;
mod splash;
mod theme;
mod util;

pub use crate::settings::Settings;
pub use workspace_rs::Event;

#[cfg(feature = "egui_wgpu_renderer")]
pub use lb_wgpu::*;

use crate::account::AccountScreen;
use crate::onboard::{OnboardHandOff, OnboardScreen};
use crate::splash::{SplashHandOff, SplashScreen};
use std::sync::{Arc, RwLock};

#[allow(clippy::large_enum_variant)]
pub enum Lockbook {
    /// The first screen a user sees everytime while the application is initializing. If there's a
    /// major error, it's shown here. If all goes well, we move on to either the Onboard screen or
    /// the Account screen, depending on whether there's a Lockbook account.
    Splash(SplashScreen),

    /// The screen that presents the user with the option to create or import a Lockbook account.
    Onboard(OnboardScreen),

    /// The user's primary Lockbook interface once they have an account.
    Account(AccountScreen),
}

#[derive(Debug, Default)]
pub struct Response {
    pub close: bool,
    pub is_dev: bool,
}

pub const DEV_USERS: &[&str] = &["parth", "adam", "travis", "at"];

impl Lockbook {
    pub fn new(ctx: &egui::Context) -> Self {
        let (settings, maybe_settings_err) = match Settings::read_from_file() {
            Ok(s) => (s, None),
            Err(err) => (Default::default(), Some(err.to_string())),
        };
        let settings = Arc::new(RwLock::new(settings));

        let splash = SplashScreen::new(settings, maybe_settings_err);
        splash.start_loading_core(ctx);
        Lockbook::Splash(splash)
    }

    // Since updating from egui 0.28 to 0.30, visuals are for some reason reset
    // between init and first frame. This fn is called during first update for
    // deferred initialization.
    pub fn deferred_init(&self, ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        workspace_rs::register_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(ctx);

        theme::init(&self.settings(), ctx);
    }

    fn settings(&self) -> Arc<RwLock<Settings>> {
        match self {
            Lockbook::Splash(screen) => screen.settings.clone(),
            Lockbook::Onboard(screen) => screen.settings.clone(),
            Lockbook::Account(screen) => screen.settings.clone(),
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Response {
        let mut output = Response::default();
        match self {
            // If we're on the Splash screen, we're waiting for the handoff to transition to the
            // Account or Onboard screen. Once we get it, we adjust the application state and
            // request a new frame.
            Self::Splash(screen) => {
                if let Some(handoff) = screen.update(ctx) {
                    let SplashHandOff { settings, core, maybe_files: maybe_acct_data } = handoff;

                    *self = match maybe_acct_data {
                        Some(files) => {
                            let is_new_user = false;
                            let acct_scr =
                                AccountScreen::new(settings, &core, files, ctx, is_new_user);
                            Self::Account(acct_scr)
                        }
                        None => Self::Onboard(OnboardScreen::new(settings, core)),
                    };

                    ctx.request_repaint();
                };
            }
            // If we're on the Onboard screen, we're waiting for the handoff to transition to the
            // Account screen.
            Self::Onboard(screen) => {
                if let Some(handoff) = screen.update(ctx) {
                    let OnboardHandOff { settings, core, acct_data, is_new_user } = handoff;

                    let acct_scr = AccountScreen::new(settings, &core, acct_data, ctx, is_new_user);
                    *self = Self::Account(acct_scr);

                    ctx.request_repaint();
                }
            }
            // On the account screen, we're just waiting for it to gracefully shutdown or for the user to log out.
            Self::Account(screen) => {
                screen.update(ctx);

                // React to the user requesting close (e.g. clicking the window's X button).
                // Priority: close an open modal first; otherwise begin graceful shutdown.
                // begin_shutdown is idempotent so it's safe to call repeatedly while the
                // platform integration keeps signaling close_requested.
                if ctx.input(|i| i.viewport().close_requested())
                    && !screen.is_shutdown()
                    && !screen.close_something()
                {
                    screen.begin_shutdown(ctx);
                }

                if screen.is_shutdown() {
                    output.close = true;
                }

                output.is_dev = screen.is_dev;
            }
        }
        output
    }
}

#[cfg(feature = "egui_wgpu_renderer")]
mod lb_wgpu {

    use egui::{PlatformOutput, ViewportIdMap, ViewportOutput};
    use egui_wgpu_renderer::RendererState;

    use crate::{Lockbook, Response};

    #[repr(C)]
    pub struct WgpuLockbook<'window> {
        pub renderer: RendererState<'window>,

        // events for the subsequent two frames, because canvas expects buttons to be down for two frames
        pub queued_events: Vec<egui::Event>,
        pub double_queued_events: Vec<egui::Event>,

        pub app: Lockbook,
    }

    #[derive(Default)]
    pub struct Output {
        // platform response
        pub platform: PlatformOutput,
        pub viewport: ViewportIdMap<ViewportOutput>,

        // widget response
        pub app: Response,
    }

    impl WgpuLockbook<'_> {
        pub fn frame(&mut self) -> Output {
            self.renderer.begin_frame();
            let app_response = self.app.update(&self.renderer.context);
            self.renderer.set_is_dev(app_response.is_dev);
            let (platform, viewport) = self.renderer.end_frame();

            // Queue up the events for the next frame
            self.renderer
                .raw_input
                .events
                .append(&mut self.queued_events);
            self.queued_events.append(&mut self.double_queued_events);
            if !self.renderer.raw_input.events.is_empty() {
                self.renderer.context.request_repaint();
            }

            Output { platform, viewport, app: app_response }
        }
    }
}
