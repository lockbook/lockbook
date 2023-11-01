#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use egui_winit::egui;
use lockbook_egui::Lockbook;
use lockbook_egui::Settings;

fn main() {
    // We explicity use x11 on posix systems because using Wayland (at least on GNOME) has the
    // following issues:
    //  1. window decorations are non-native.
    //  2. dragging & dropping from the system doesn't work.
    if std::env::var_os("WINIT_UNIX_BACKEND").is_none() {
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    }

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
            Box::new(Lockbook::new(&cc.egui_ctx, settings, maybe_settings_err))
        }),
    )
    .unwrap();
}
