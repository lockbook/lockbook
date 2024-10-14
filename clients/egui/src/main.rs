#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Cursor;

use egui_winit::egui;
use image::ImageDecoder as _;
use lockbook_egui::Lockbook;
use lockbook_egui::Settings;

fn main() {
    env_logger::init();

    // We explicity use x11 on posix systems because using Wayland (at least on GNOME) has the
    // following issues:
    //  1. window decorations are non-native.
    //  2. dragging & dropping from the system doesn't work.
    // if std::env::var_os("WINIT_UNIX_BACKEND").is_none() {
    //     std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    // }

    // We load the settings this early because some of them adjust certain launch behaviors such
    // as maximizing the window on startup or theming. For example, a user's splash screen should
    // conform to their theme choice.
    let (settings, maybe_settings_err) = match Settings::read_from_file() {
        Ok(s) => (s, None),
        Err(err) => (Settings::default(), Some(err.to_string())),
    };

    let icon_bytes = {
        let png_bytes = include_bytes!("../lockbook.png");

        let decoder = image::codecs::png::PngDecoder::new(Cursor::new(png_bytes))
            .expect("Failed to create PNG decoder");
        let mut rgba8_bytes = vec![0; decoder.total_bytes() as usize];
        decoder
            .read_image(&mut rgba8_bytes)
            .expect("Failed to read PNG image");

        rgba8_bytes
    };

    eframe::run_native(
        "Lockbook",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_drag_and_drop(true)
                .with_maximized(settings.window_maximize)
                .with_inner_size([1300.0, 800.0])
                .with_icon(egui::IconData { rgba: icon_bytes, width: 128, height: 128 }),
            ..Default::default()
        },
        Box::new(|cc: &eframe::CreationContext| {
            Ok(Box::new(Lockbook::new(&cc.egui_ctx, settings, maybe_settings_err)))
        }),
    )
    .unwrap();
}
