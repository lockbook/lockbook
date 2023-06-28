mod icons;
mod palette;
mod visuals;

pub use icons::Icon;
pub use palette::{DrawingPalette, ThemePalette};

use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use eframe::egui;

use crate::settings::{Settings, ThemeMode};

pub fn init(s: &Arc<RwLock<Settings>>, ctx: &egui::Context) {
    let initial_mode = match s.read().unwrap().theme_mode {
        ThemeMode::System => dark_light::detect(),
        ThemeMode::Dark => dark_light::Mode::Dark,
        ThemeMode::Light => dark_light::Mode::Light,
    };

    let primary = s.read().unwrap().theme_color;

    ctx.set_visuals(egui_visuals(initial_mode, primary));

    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = egui::vec2(7.0, 7.0);
    style.spacing.menu_margin = egui::Margin::same(10.0);

    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::new(17.0, egui::FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::new(17.0, egui::FontFamily::Monospace));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::new(17.0, egui::FontFamily::Proportional));
    ctx.set_style(style);

    poll_system_theme(s, ctx, initial_mode);
}

pub fn apply_settings(s: &Settings, ctx: &egui::Context) {
    let mode = match s.theme_mode {
        ThemeMode::System => dark_light::detect(),
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
            let m = dark_light::detect();
            if mode != m {
                mode = m;
                ctx.set_visuals(egui_visuals(m, s.read().unwrap().theme_color));
                ctx.request_repaint();
            }
        }
        thread::sleep(Duration::from_secs(1));
    });
}

pub fn egui_visuals(m: dark_light::Mode, primary: lb::ColorAlias) -> egui::Visuals {
    match m {
        dark_light::Mode::Default | dark_light::Mode::Dark => visuals::dark(primary),
        dark_light::Mode::Light => visuals::light(primary),
    }
}

pub fn register_fonts(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        "material_icons".to_owned(),
        egui::FontData::from_static(icons::MATERIAL_ICON_FONT),
    );

    fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .push("material_icons".to_owned());
}
