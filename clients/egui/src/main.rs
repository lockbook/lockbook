#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Cursor;
use std::ops::DerefMut;

use egui::ViewportCommand;
use image::ImageDecoder as _;
use lockbook_egui::Lockbook;

fn main() {
    env_logger::init();

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
                .with_icon(egui::IconData { rgba: icon_bytes, width: 128, height: 128 })
                .with_titlebar_shown(false)
                .with_title_shown(false)
                .with_fullsize_content_view(true),
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
                deferred_init_done: false,
            }))
        }),
    )
    .unwrap();
}

struct EframeLockbook {
    lb: Lockbook,
    deferred_init_done: bool,
}

impl eframe::App for EframeLockbook {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.deferred_init_done {
            self.lb.deferred_init(ctx);
            self.deferred_init_done = true;
        }

        let output = self.lb.update(ctx);

        // Veto eframe's close-on-X until the app itself asks to close.
        if !output.close && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
    }
}
