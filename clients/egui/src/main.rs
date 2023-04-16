#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod model;
mod onboard;
mod settings;
mod splash;
mod theme;
mod util;
mod widgets;

use std::sync::{Arc, RwLock};

use eframe::egui;

use crate::account::AccountScreen;
use crate::onboard::{OnboardHandOff, OnboardScreen};
use crate::settings::Settings;
use crate::splash::{SplashHandOff, SplashScreen};

fn main() {
    // We explicity use x11 on posix systems because using Wayland (at least on GNOME) has the
    // following issues:
    //  1. window decorations are non-native.
    //  2. dragging & dropping from the system doesn't work.
    std::env::set_var("WINIT_UNIX_BACKEND", "x11");

    // We load the settings this early because some of them adjust certain launch behaviors such
    // as maximizing the window on startup or theming. For example, a user's splash screen should
    // conform to their theme choice.
    let (settings, maybe_settings_err) = match Settings::read_from_file() {
        Ok(s) => (s, None),
        Err(err) => (Settings::default(), Some(err.to_string())),
    };

    eframe::run_native(
        "Lockbook",
        eframe::NativeOptions {
            drag_and_drop_support: true,
            maximized: settings.window_maximize,
            initial_window_size: Some(egui::vec2(1300.0, 800.0)),
            icon_data: Some(eframe::IconData {
                rgba: include_bytes!("../lockbook.ico").to_vec(),
                width: 32,
                height: 32,
            }),
            ..Default::default()
        },
        Box::new(|cc: &eframe::CreationContext| {
            let settings = Arc::new(RwLock::new(settings));

            let mut fonts = egui::FontDefinitions::default();
            lbeditor::register_fonts(&mut fonts);
            theme::register_fonts(&mut fonts);
            cc.egui_ctx.set_fonts(fonts);

            theme::init(&settings, &cc.egui_ctx);

            let splash = SplashScreen::new(settings, maybe_settings_err);
            splash.start_loading_core(&cc.egui_ctx);
            Box::new(Lockbook::Splash(splash))
        }),
    )
    .unwrap();
}

enum Lockbook {
    /// The first screen a user sees everytime while the application is initializing. If there's a
    /// major error, it's shown here. If all goes well, we move on to either the Onboard screen or
    /// the Account screen, depending on whether there's a Lockbook account.
    Splash(SplashScreen),

    /// The screen that presents the user with the option to create or import a Lockbook account.
    Onboard(Box<OnboardScreen>),

    /// The user's primary Lockbook interface once they have an account.
    Account(Box<AccountScreen>),
}

impl eframe::App for Lockbook {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match self {
            // If we're on the Splash screen, we're waiting for the handoff to transition to the
            // Account or Onboard screen. Once we get it, we adjust the application state and
            // request a new frame.
            Self::Splash(screen) => {
                if let Some(handoff) = screen.update(ctx) {
                    let SplashHandOff { settings, core, maybe_acct_data } = handoff;

                    *self = match maybe_acct_data {
                        Some(acct_data) => {
                            let acct_scr = AccountScreen::new(settings, core, acct_data);
                            Self::Account(Box::new(acct_scr))
                        }
                        None => Self::Onboard(Box::new(OnboardScreen::new(settings, core))),
                    };

                    ctx.request_repaint();
                }
            }
            // If we're on the Onboard screen, we're waiting for the handoff to transition to the
            // Account screen.
            Self::Onboard(screen) => {
                if let Some(handoff) = screen.update(ctx) {
                    let OnboardHandOff { settings, core, acct_data } = handoff;

                    let acct_scr = AccountScreen::new(settings, core, acct_data);
                    *self = Self::Account(Box::new(acct_scr));

                    ctx.request_repaint();
                }
            }
            // We don't have any responses to process from the Account screen. However, if we were
            // to implement something like the ability for a user to wipe their account locally,
            // we'd take care of that here.
            Self::Account(screen) => screen.update(ctx, frame),
        }
    }

    /// We override `on_close_event` in order to give the Account screen a chance to close any open
    /// modals or dialogs via a window close event.
    fn on_close_event(&mut self) -> bool {
        match self {
            Self::Account(screen) => {
                if !screen.close_something() {
                    screen.save_all_tabs();
                    true
                } else {
                    false
                }
            }
            _ => true,
        }
    }
}
