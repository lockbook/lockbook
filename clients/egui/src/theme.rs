use dark_light::Mode::Dark;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use workspace_rs::theme::palette::ColorAlias;
use workspace_rs::theme::visuals;

use crate::settings::{Settings, ThemeMode};

pub fn init(s: &Arc<RwLock<Settings>>, ctx: &egui::Context) {
    let initial_mode = match s.read().unwrap().theme_mode {
        ThemeMode::System => dark_light::detect().unwrap(),
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
        ThemeMode::System => dark_light::detect().unwrap(),
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

    thread::spawn(move || loop {
        if s.read().unwrap().theme_mode == ThemeMode::System {
            let m = dark_light::detect().unwrap();
            if mode != m {
                mode = m;
                ctx.set_visuals(egui_visuals(m, s.read().unwrap().theme_color));
                ctx.request_repaint();
            }
        }
        thread::sleep(Duration::from_secs(1));
    });
}

pub fn egui_visuals(m: dark_light::Mode, primary: ColorAlias) -> egui::Visuals {
    match m {
        dark_light::Mode::Dark => visuals::dark(primary),
        dark_light::Mode::Unspecified | dark_light::Mode::Light => visuals::light(primary),
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
