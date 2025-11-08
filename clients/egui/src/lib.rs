#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod model;
mod onboard;
mod settings;
mod splash;
mod theme;
mod util;

pub use crate::settings::Settings;
pub use workspace_rs::Event;

#[cfg(feature = "egui_wgpu_backend")]
pub use lb_wgpu::*;

use crate::account::AccountScreen;
use crate::onboard::{OnboardHandOff, OnboardScreen};
use crate::splash::{SplashHandOff, SplashScreen};
use std::sync::{Arc, RwLock};

#[allow(clippy::large_enum_variant)]
pub enum Lockbook {
    /// The first screen a user sees everytime while the application is initializing. If there's a
    /// major error, it's shown here. If all goes well, we move on to either the Onboard screen or
    /// the Account screen, depending on whether there's a Lockbook account.
    Splash(SplashScreen),

    /// The screen that presents the user with the option to create or import a Lockbook account.
    Onboard(OnboardScreen),

    /// The user's primary Lockbook interface once they have an account.
    Account(AccountScreen),
}

#[derive(Debug, Default)]
pub struct Response {
    pub close: bool,
}

impl Lockbook {
    pub fn new(
        ctx: &egui::Context, settings: Settings, maybe_settings_err: Option<String>,
    ) -> Self {
        let settings = Arc::new(RwLock::new(settings));

        let mut fonts = egui::FontDefinitions::default();
        workspace_rs::register_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(ctx);

        theme::init(&settings, ctx);

        let splash = SplashScreen::new(settings, maybe_settings_err);
        splash.start_loading_core(ctx);
        Lockbook::Splash(splash)
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Response {
        let mut output = Response::default();
        match self {
            // If we're on the Splash screen, we're waiting for the handoff to transition to the
            // Account or Onboard screen. Once we get it, we adjust the application state and
            // request a new frame.
            Self::Splash(screen) => {
                if let Some(handoff) = screen.update(ctx) {
                    let SplashHandOff { settings, core, maybe_acct_data } = handoff;

                    *self = match maybe_acct_data {
                        Some(acct_data) => {
                            let is_new_user = false;
                            let acct_scr =
                                AccountScreen::new(settings, &core, acct_data, ctx, is_new_user);
                            Self::Account(acct_scr)
                        }
                        None => Self::Onboard(OnboardScreen::new(settings, core)),
                    };

                    ctx.request_repaint();
                };
            }
            // If we're on the Onboard screen, we're waiting for the handoff to transition to the
            // Account screen.
            Self::Onboard(screen) => {
                if let Some(handoff) = screen.update(ctx) {
                    let OnboardHandOff { settings, core, acct_data } = handoff;

                    let is_new_user = true;
                    let acct_scr = AccountScreen::new(settings, &core, acct_data, ctx, is_new_user);
                    *self = Self::Account(acct_scr);

                    ctx.request_repaint();
                }
            }
            // On the account screen, we're just waiting for it to gracefully shutdown or for the user to log out.
            Self::Account(screen) => {
                screen.update(ctx);
                if screen.is_shutdown() {
                    output.close = true;
                }
                if screen.is_shutdown() {
                    output.close = true;
                }
            }
        }
        output
    }
}

#[cfg(feature = "egui_wgpu_backend")]
mod lb_wgpu {
    use std::iter;
    use std::time::Instant;

    use egui::{PlatformOutput, Pos2, Rect, ViewportIdMap, ViewportOutput};
    use egui_wgpu_backend::wgpu::{self, CompositeAlphaMode};

    use crate::{Lockbook, Response};

    #[repr(C)]
    pub struct WgpuLockbook<'window> {
        pub start_time: Instant,

        pub device: wgpu::Device,
        pub queue: wgpu::Queue,
        pub surface: wgpu::Surface<'window>,
        pub adapter: wgpu::Adapter,

        // remember size last frame to detect resize
        pub surface_width: u32,
        pub surface_height: u32,

        pub rpass: egui_wgpu_backend::RenderPass,
        pub screen: egui_wgpu_backend::ScreenDescriptor,

        pub context: egui::Context,
        pub raw_input: egui::RawInput,

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
            self.configure_surface();
            let output_frame = match self.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Outdated) => {
                    // This error occurs when the app is minimized on Windows.
                    // Silently return here to prevent spamming the console with:
                    // "The underlying surface has changed, and therefore the swap chain must be updated"
                    return Default::default();
                }
                Err(e) => {
                    eprintln!("Dropped frame with error: {e}");
                    return Default::default();
                }
            };

            let output_view = output_frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            #[cfg(not(target_os = "linux"))]
            let msaa_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("msaa_texture"),
                size: output_frame.texture.size(),
                mip_level_count: output_frame.texture.mip_level_count(),
                sample_count: 4,
                dimension: output_frame.texture.dimension(),
                format: output_frame.texture.format(),
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            #[cfg(not(target_os = "linux"))]
            let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

            // can probably use run
            self.set_egui_screen();
            self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
            self.context.begin_frame(self.raw_input.take());
            let app_response = self.app.update(&self.context);
            let full_output = self.context.end_frame();
            let paint_jobs = self
                .context
                .tessellate(full_output.shapes, full_output.pixels_per_point);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });

            let tdelta: egui::TexturesDelta = full_output.textures_delta;
            self.rpass
                .add_textures(&self.device, &self.queue, &tdelta)
                .expect("add texture ok");

            self.rpass
                .update_buffers(&self.device, &self.queue, &paint_jobs, &self.screen);

            // Record all render passes.
            {
                let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }

            #[cfg(not(target_os = "linux"))]
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &msaa_view,
                        resolve_target: Some(&output_view),
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                self.rpass
                    .execute_with_renderpass(&mut pass, &paint_jobs, &self.screen)
                    .unwrap();
            }
            #[cfg(target_os = "linux")]
            {
                self.rpass
                    .execute(
                        &mut encoder,
                        &output_view,
                        &paint_jobs,
                        &self.screen,
                        Some(wgpu::Color::BLACK),
                    )
                    .unwrap();
            }

            // Submit the commands.
            self.queue.submit(iter::once(encoder.finish()));

            // Redraw egui
            output_frame.present();

            self.rpass
                .remove_textures(tdelta)
                .expect("remove texture ok");

            // Queue up the events for the next frame
            self.raw_input.events.append(&mut self.queued_events);
            self.queued_events.append(&mut self.double_queued_events);
            if !self.raw_input.events.is_empty() {
                self.context.request_repaint();
            }

            Output {
                platform: full_output.platform_output,
                viewport: full_output.viewport_output,
                app: app_response,
            }
        }

        pub fn set_egui_screen(&mut self) {
            self.raw_input.screen_rect = Some(Rect {
                min: Pos2::ZERO,
                max: Pos2::new(
                    self.screen.physical_width as f32 / self.screen.scale_factor,
                    self.screen.physical_height as f32 / self.screen.scale_factor,
                ),
            });
        }

        pub fn surface_format(&self) -> wgpu::TextureFormat {
            // todo: is this really fine?
            // from here: https://github.com/hasenbanck/egui_example/blob/master/src/main.rs#L65
            println!("{:#?}", self.surface.get_capabilities(&self.adapter));
            self.surface.get_capabilities(&self.adapter).formats[0]
        }

        pub fn configure_surface(&mut self) {
            let resized = self.screen.physical_width != self.surface_width
                || self.screen.physical_height != self.surface_height;
            let visible = self.screen.physical_width * self.screen.physical_height != 0;
            if resized && visible {
                let surface_config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: self.surface_format(),
                    width: self.screen.physical_width,
                    height: self.screen.physical_height,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: CompositeAlphaMode::Auto,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                };
                self.surface.configure(&self.device, &surface_config);
                self.surface_width = self.screen.physical_width;
                self.surface_height = self.screen.physical_height;
            }
        }
    }
}
