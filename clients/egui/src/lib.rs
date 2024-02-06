#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod model;
mod onboard;
mod settings;
mod splash;
mod theme;
mod util;

pub use crate::settings::Settings;

use crate::account::AccountScreen;
use crate::onboard::{OnboardHandOff, OnboardScreen};
use crate::splash::{SplashHandOff, SplashScreen};
use eframe::egui;
use egui_wgpu_backend::wgpu::{self, CompositeAlphaMode};
use egui_winit::egui::{PlatformOutput, Pos2, Rect};
use std::iter;
use std::sync::{Arc, RwLock};
use std::time::Instant;

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
pub struct UpdateOutput {
    pub close: bool,
    pub set_window_title: Option<String>,
}

impl Lockbook {
    pub fn new(
        ctx: &egui::Context, settings: Settings, maybe_settings_err: Option<String>,
    ) -> Self {
        let settings = Arc::new(RwLock::new(settings));

        let mut fonts = egui::FontDefinitions::default();
        workspace_rs::register_fonts(&mut fonts);
        theme::register_fonts(&mut fonts);
        ctx.set_fonts(fonts);

        theme::init(&settings, ctx);

        let splash = SplashScreen::new(settings, maybe_settings_err);
        splash.start_loading_core(ctx);
        Lockbook::Splash(splash)
    }

    pub fn update(&mut self, ctx: &egui::Context) -> UpdateOutput {
        let mut output = Default::default();
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
                                AccountScreen::new(settings, core, acct_data, ctx, is_new_user);
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
                    let acct_scr = AccountScreen::new(settings, core, acct_data, ctx, is_new_user);
                    *self = Self::Account(acct_scr);

                    ctx.request_repaint();
                }
            }
            // On the account screen, we're just waiting for it to gracefully shutdown.
            Self::Account(screen) => {
                screen.update(ctx, &mut output);
                if screen.is_shutdown() {
                    output.close = true;
                }
            }
        }
        output
    }
}

impl eframe::App for Lockbook {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let output = Lockbook::update(self, ctx);
        if output.close {
            frame.close();
        }
        if let Some(set_window_title) = output.set_window_title {
            frame.set_window_title(&set_window_title);
        }
    }

    /// We override `on_close_event` in order to give the Account screen a chance to:
    /// 1) close any open modals or dialogs via a window close event, or
    /// 2) to start a graceful shutdown by saving state and cleaning up.
    fn on_close_event(&mut self) -> bool {
        match self {
            Self::Account(screen) => {
                // If the account screen is done shutting down, it's safe to close the app.
                if screen.is_shutdown() {
                    return true;
                }
                // If the account screen didn't close an open modal, we begin the shutdown process.
                if !screen.close_something() {
                    screen.begin_shutdown();
                }
                false
            }
            _ => true,
        }
    }
}

#[repr(C)]
pub struct WgpuLockbook {
    pub start_time: Instant,

    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,

    pub rpass: egui_wgpu_backend::RenderPass,
    pub screen: egui_wgpu_backend::ScreenDescriptor,

    pub context: egui::Context,
    pub raw_input: egui::RawInput,

    pub from_host: Option<String>,
    pub from_egui: Option<String>,

    pub app: Lockbook,
}

#[derive(Default)]
pub struct IntegrationOutput {
    pub redraw_in: u64,
    pub egui: PlatformOutput,
    pub update_output: UpdateOutput,
}

impl WgpuLockbook {
    pub fn frame(&mut self) -> IntegrationOutput {
        let mut out = IntegrationOutput::default();
        self.configure_surface();
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                return out;
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                return out;
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // can probably use run
        self.set_egui_screen();
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.context.begin_frame(self.raw_input.take());
        out.update_output = self.app.update(&self.context);
        let full_output = self.context.end_frame();
        if !full_output.platform_output.copied_text.is_empty() {
            // todo: can this go in output?
            self.from_egui = Some(full_output.platform_output.copied_text.clone());
        }
        let paint_jobs = self.context.tessellate(full_output.shapes);
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
        self.rpass
            .execute(
                &mut encoder,
                &output_view,
                &paint_jobs,
                &self.screen,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();
        // Submit the commands.
        self.queue.submit(iter::once(encoder.finish()));

        // Redraw egui
        output_frame.present();

        self.rpass
            .remove_textures(tdelta)
            .expect("remove texture ok");

        out.redraw_in = full_output.repaint_after.as_millis() as u64;
        out.egui = full_output.platform_output;
        out
    }

    pub fn set_egui_screen(&mut self) {
        self.raw_input.screen_rect = Some(Rect {
            min: Pos2::ZERO,
            max: Pos2::new(
                self.screen.physical_width as f32 / self.screen.scale_factor,
                self.screen.physical_height as f32 / self.screen.scale_factor,
            ),
        });
        self.raw_input.pixels_per_point = Some(self.screen.scale_factor);
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        // todo: is this really fine?
        // from here: https://github.com/hasenbanck/egui_example/blob/master/src/main.rs#L65
        self.surface.get_capabilities(&self.adapter).formats[0]
    }

    pub fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format(),
            width: self.screen.physical_width,
            height: self.screen.physical_height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        if surface_config.width * surface_config.height != 0 {
            self.surface.configure(&self.device, &surface_config);
        }
    }
}
