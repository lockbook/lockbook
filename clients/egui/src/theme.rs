use dark_light::Mode::Dark;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use workspace_rs::theme::palette::ColorAlias;
use workspace_rs::theme::visuals;

use crate::settings::{Settings, ThemeMode};

pub fn init(s: &Arc<RwLock<Settings>>, ctx: &egui::Context) {
    let initial_mode = match s.read().unwrap().theme_mode {
        ThemeMode::System => detect_theme_wrapper(),
        ThemeMode::Dark => dark_light::Mode::Dark,
        ThemeMode::Light => dark_light::Mode::Light,
    };

    let primary = s.read().unwrap().theme_color;

    ctx.set_visuals(egui_visuals(initial_mode, primary));

    visuals::init(ctx, initial_mode == Dark);

    poll_system_theme(s, ctx, initial_mode);
}

pub fn apply_settings(s: &Settings, ctx: &egui::Context) {
    let mode = match s.theme_mode {
        ThemeMode::System => detect_theme_wrapper(),
        ThemeMode::Dark => dark_light::Mode::Dark,
        ThemeMode::Light => dark_light::Mode::Light,
    };

    ctx.set_visuals(egui_visuals(mode, s.theme_color));
}

fn poll_system_theme(
    s: &Arc<RwLock<Settings>>, ctx: &egui::Context, initial_mode: dark_light::Mode,
) {
    let s = s.clone();
    let ctx = ctx.clone();

    let mut mode = initial_mode;

    thread::spawn(move || {
        loop {
            if s.read().unwrap().theme_mode == ThemeMode::System {
                match dark_light::detect() {
                    Ok(m) => {
                        if mode != m {
                            mode = m;
                            ctx.set_visuals(egui_visuals(m, s.read().unwrap().theme_color));
                            ctx.request_repaint();
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to detect current dark/light mode: {e:?}")
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn detect_theme_wrapper() -> dark_light::Mode {
    dark_light::detect().unwrap_or_else(|err| {
        eprintln!("Failed to detect current dark/light mode: {err:?} (2)");
        dark_light::Mode::Unspecified
    })
}

pub fn egui_visuals(m: dark_light::Mode, primary: ColorAlias) -> egui::Visuals {
    match m {
        // the default mode of operation is light because on gnome, by default, there is no
        // light mode, it is either "Default" (which is presented to us as Unspecified) or
        // dark. This "Default" mode is also illustrated as a mix of light and dark windows
        dark_light::Mode::Unspecified | dark_light::Mode::Light => visuals::light(primary),
        dark_light::Mode::Dark => visuals::dark(primary),
    }
}

pub fn register_fonts(fonts: &mut egui::FontDefinitions) {
    let mut font = egui::FontData::from_static(lb_fonts::MATERIAL_SYMBOLS_OUTLINED);
    font.tweak.y_offset_factor = -0.1;

    fonts.font_data.insert("material_icons".to_owned(), font);

    fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .push("material_icons".to_owned());
}
