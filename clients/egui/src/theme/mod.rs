pub mod icons;
pub mod tokens;

use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};

use crate::settings::{Settings, ThemeMode, ensure_themes_dir, load_theme};

/// Apply mode + color theme from settings onto the egui context.
pub fn apply_settings(s: &Settings, ctx: &egui::Context, os_dark: bool) {
    ensure_themes_dir();
    let mode = resolve_mode(s.theme_mode, os_dark);
    let theme = load_theme(&s.theme_name, mode).unwrap_or_else(|| Theme::default(mode));
    ctx.set_lb_theme(theme);
    ctx.request_repaint();
}

/// Resolve light/dark from preference + current OS appearance.
pub fn resolve_mode(pref: ThemeMode, os_dark: bool) -> Mode {
    match pref {
        ThemeMode::System => {
            if os_dark {
                Mode::Dark
            } else {
                Mode::Light
            }
        }
        ThemeMode::Dark => Mode::Dark,
        ThemeMode::Light => Mode::Light,
    }
}
