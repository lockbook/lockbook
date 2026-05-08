use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};
use workspace_rs::theme::visuals;

use crate::settings::{Settings, ThemeMode, ensure_themes_dir, load_theme};

pub fn init(s: &Arc<RwLock<Settings>>, ctx: &egui::Context) {
    ensure_themes_dir();

    let settings = s.read().unwrap();
    let initial_mode = match settings.theme_mode {
        ThemeMode::System => detect_theme_wrapper(),
        ThemeMode::Dark => dark_light::Mode::Dark,
        ThemeMode::Light => dark_light::Mode::Light,
    };

    set_colors(ctx, initial_mode, &settings.theme_name);
    drop(settings);
    visuals::init(ctx);

    poll_system_theme(s, ctx, initial_mode);
}

pub fn apply_settings(s: &Settings, ctx: &egui::Context) {
    let mode = match s.theme_mode {
        ThemeMode::System => detect_theme_wrapper(),
        ThemeMode::Dark => dark_light::Mode::Dark,
        ThemeMode::Light => dark_light::Mode::Light,
    };

    set_colors(ctx, mode, &s.theme_name);
}

fn poll_system_theme(
    s: &Arc<RwLock<Settings>>, ctx: &egui::Context, initial_mode: dark_light::Mode,
) {
    let s = s.clone();
    let ctx = ctx.clone();

    let mut mode = initial_mode;
    let mut last_error_logged = false;

    thread::spawn(move || {
        loop {
            let settings = s.read().unwrap();
            if settings.theme_mode == ThemeMode::System {
                match dark_light::detect() {
                    Ok(m) => {
                        last_error_logged = false;
                        if mode != m {
                            mode = m;
                            set_colors(&ctx, m, &settings.theme_name);

                            // since updating from egui 0.28 to 0.30, this is
                            // necessary to prevent light/dark mode switching
                            // from resetting font sizes to default
                            visuals::init(&ctx);

                            ctx.request_repaint();
                        }
                    }
                    Err(e) => {
                        // Only log the error once to avoid spamming
                        if !last_error_logged {
                            eprintln!("Failed to detect current dark/light mode: {e:?}");
                            last_error_logged = true;
                        }
                    }
                }
            }
            drop(settings);

            thread::sleep(Duration::from_secs(2));
        }
    });
}

fn detect_theme_wrapper() -> dark_light::Mode {
    dark_light::detect().unwrap_or_else(|err| {
        eprintln!("Failed to detect current dark/light mode: {err:?} (2)");
        dark_light::Mode::Unspecified
    })
}

pub fn set_colors(ctx: &egui::Context, m: dark_light::Mode, theme_name: &str) {
    let mode = match m {
        // the default mode of operation is light because on gnome, by default, there is no
        // light mode, it is either "Default" (which is presented to us as Unspecified) or
        // dark. This "Default" mode is also illustrated as a mix of light and dark windows
        dark_light::Mode::Unspecified | dark_light::Mode::Light => Mode::Light,
        dark_light::Mode::Dark => Mode::Dark,
    };

    let theme = load_theme(theme_name, mode).unwrap_or_else(|| Theme::default(mode));
    ctx.set_lb_theme(theme);
}
