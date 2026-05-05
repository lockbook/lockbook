#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{io::Cursor, ops::DerefMut};

use egui::ViewportCommand;
use egui_winit::egui;
use image::ImageDecoder as _;
use lockbook_egui::Lockbook;

fn main() {
    env_logger::init();

    // We explicity use x11 on posix systems because using Wayland (at least on GNOME) has the
    // following issues:
    //  1. window decorations are non-native.
    //  2. dragging & dropping from the system doesn't work.
    // if std::env::var_os("WINIT_UNIX_BACKEND").is_none() {
    //     std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    // }

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
                .with_inner_size([1300.0, 800.0])
                .with_icon(egui::IconData { rgba: icon_bytes, width: 128, height: 128 }),
            ..Default::default()
        },
        Box::new(|cc: &eframe::CreationContext| {
            let Some(ref wgpu) = cc.wgpu_render_state else {
                panic!("must use wgpu as graphics target")
            };
            workspace_rs::register_render_callback_resources(
                &wgpu.device,
                &wgpu.queue,
                wgpu.target_format,
                wgpu.renderer.write().deref_mut(),
                workspace_rs::register_font_system(&cc.egui_ctx),
                1,
            );

            Ok(Box::new(EframeLockbook {
                lb: Lockbook::new(&cc.egui_ctx),
                deferred_init_completed: false,
            }))
        }),
    )
    .unwrap();
}

struct EframeLockbook {
    lb: Lockbook,
    deferred_init_completed: bool,
}

impl eframe::App for EframeLockbook {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.deferred_init_completed {
            self.lb.deferred_init(ctx);
            self.deferred_init_completed = true;
        }

        let output = self.lb.update(ctx);

        // While the app is shutting down (or hasn't started shutting down despite
        // close_requested), veto eframe's default close-on-X behavior so it can run
        // to completion. Once `output.close` flips true, we let the close proceed.
        if !output.close && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
    }
}
