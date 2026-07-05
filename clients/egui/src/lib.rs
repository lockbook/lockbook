#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod design_system;
mod settings;
mod theme;
mod util;
mod widgets;

pub use crate::settings::Settings;

#[cfg(feature = "egui_wgpu_renderer")]
pub use lb_wgpu::*;

use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};
use workspace_rs::theme::visuals;

pub struct Lockbook {
    mode: Mode,
}

#[derive(Debug, Default)]
pub struct Response {
    pub close: bool,
}

impl Lockbook {
    pub fn new(ctx: &egui::Context) -> Self {
        let mode = if ctx.style().visuals.dark_mode { Mode::Dark } else { Mode::Light };
        Lockbook { mode }
    }

    /// Deferred one-time setup: fonts, image loaders, and the color theme. egui
    /// resets visuals between init and the first frame, so this runs on frame one.
    pub fn deferred_init(&self, ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        workspace_rs::register_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(ctx);

        let mode = if ctx.style().visuals.dark_mode { Mode::Dark } else { Mode::Light };
        ctx.set_lb_theme(Theme::default(mode));
        visuals::init(ctx);
    }

    pub fn is_dev(&self) -> bool {
        false
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Response {
        let tokens = theme::tokens::Tokens::new(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(tokens.canvas()))
            .show(ctx, |ui| {
                // Clearance for the macOS traffic lights (no title bar).
                ui.add_space(20.0);
                egui::Frame::default()
                    .inner_margin(egui::Margin::symmetric(60, 12))
                    .show(ui, |ui| {
                        design_system::show(ctx, ui, &tokens, &mut self.mode);
                    });
            });

        Response { close: ctx.input(|i| i.viewport().close_requested()) }
    }
}

#[cfg(feature = "egui_wgpu_renderer")]
mod lb_wgpu {
    use egui::{PlatformOutput, ViewportIdMap, ViewportOutput};
    use egui_wgpu_renderer::RendererState;

    use crate::{Lockbook, Response};

    #[repr(C)]
    pub struct WgpuLockbook<'window> {
        pub renderer: RendererState<'window>,

        // events for the subsequent two frames, because canvas expects buttons to be down for two frames
        pub queued_events: Vec<egui::Event>,
        pub double_queued_events: Vec<egui::Event>,

        pub app: Lockbook,
    }

    #[derive(Default)]
    pub struct Output {
        // platform response
        pub platform: PlatformOutput,
        pub viewport: ViewportIdMap<ViewportOutput>,

        // widget response
        pub app: Response,
    }

    impl WgpuLockbook<'_> {
        pub fn frame(&mut self) -> Output {
            self.renderer.begin_frame();
            let app_response = self.app.update(&self.renderer.context);
            self.renderer.set_is_dev(self.app.is_dev());
            let (platform, viewport) = self.renderer.end_frame();

            // Queue up the events for the next frame
            self.renderer
                .raw_input
                .events
                .append(&mut self.queued_events);
            self.queued_events.append(&mut self.double_queued_events);
            if !self.renderer.raw_input.events.is_empty() {
                self.renderer.context.request_repaint();
            }

            Output { platform, viewport, app: app_response }
        }
    }
}
