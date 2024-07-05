use egui_wgpu_backend::wgpu;
use std::time::Instant;

mod cursor_icon;

use workspace_rs::{tab::ExtendedOutput, workspace::Workspace};

#[cfg(target_vendor = "apple")]
pub mod apple;

/// cbindgen:ignore
#[cfg(target_os = "android")]
pub mod android;

/// cbindgen:ignore
#[cfg(target_os = "android")]
pub use android::resp::*;

#[cfg(not(target_os = "android"))]
pub mod resp;

#[cfg(not(target_os = "android"))]
pub use resp::*;

#[repr(C)]
pub struct WgpuWorkspace<'window> {
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

    pub workspace: Workspace,
}

#[cfg(any(target_vendor = "apple", target_os = "android"))]
impl<'window> WgpuWorkspace<'window> {
    pub fn frame(&mut self) -> IntegrationOutput {
        #[cfg(not(target_os = "android"))]
        use std::ffi::CString;
        use std::iter;

        self.configure_surface();
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                eprintln!("wgpu::SurfaceError::Outdated");
                return Default::default();
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                return Default::default();
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // can probably use run
        self.set_egui_screen();
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.context.begin_frame(self.raw_input.take());

        let workspace_response = self.workspace.draw(&self.context).into();

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

        #[cfg(not(target_os = "android"))]
        {
            if !full_output.platform_output.copied_text.is_empty() {
                // todo: can this go in output?
                out.copied_text = CString::new(full_output.platform_output.copied_text)
                    .unwrap()
                    .into_raw();
            }

            if let Some(url) = full_output.platform_output.open_url {
                out.url_opened = CString::new(url.url).unwrap().into_raw();
            }

            out.cursor = full_output.platform_output.cursor_icon.into();
        }

        #[cfg(target_os = "android")]
        {
            if let Some(url) = full_output.platform_output.open_url {
                out.url_opened = url.url;
            }

            out.has_copied_text = !full_output.platform_output.copied_text.is_empty();

            if out.has_copied_text {
                out.copied_text = full_output.platform_output.copied_text;
            }
        }

        out.redraw_in = match full_output
            .viewport_output
            .values()
            .next()
            .map(|v| v.repaint_delay)
        {
            Some(d) => d.as_millis() as u64,
            None => {
                eprintln!("VIEWPORT Missing, not requesting redraw");
                u64::max_value()
            }
        };

        #[cfg(not(target_os = "android"))]
        let copied_text = {
            let ct = full_output.platform_output.copied_text;
            if ct.is_empty() {
                std::ptr::null_mut()
            } else {
                CString::new(ct).unwrap().into_raw()
            }
        };

        #[cfg(target_os = "android")]
        let copied_text = full_output.platform_output.copied_text;

        #[cfg(not(target_os = "android"))]
        let url_opened = {
            let url = full_output.platform_output.open_url;
            if let Some(url) = url {
                CString::new(url.url).unwrap().into_raw()
            } else {
                std::ptr::null_mut()
            }
        };

        #[cfg(target_os = "android")]
        let url_opened = full_output
            .platform_output
            .open_url
            .map(|url| url.url)
            .unwrap_or_default();

        let cursor = full_output.platform_output.cursor_icon.into();

        let virtual_keyboard_shown = self.context.pop_virtual_keyboard_shown();
        let virtual_keyboard_shown_set = virtual_keyboard_shown.is_some();
        let virtual_keyboard_shown_val = virtual_keyboard_shown.unwrap_or_default();

        // todo: export context_menu_pos via self.context.pop_context_menu_pos()

        IntegrationOutput {
            workspace: workspace_response,
            redraw_in,
            copied_text,
            url_opened,
            cursor,
            virtual_keyboard_shown_set,
            virtual_keyboard_shown_val,
        }
    }

    pub fn set_egui_screen(&mut self) {
        use egui::{Pos2, Rect};

        self.raw_input.screen_rect = Some(Rect {
            min: Pos2::ZERO,
            max: Pos2::new(
                self.screen.physical_width as f32 / self.screen.scale_factor,
                self.screen.physical_height as f32 / self.screen.scale_factor,
            ),
        });
        self.context.set_pixels_per_point(self.screen.scale_factor);
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        // todo: is this really fine?
        // from here: https://github.com/hasenbanck/egui_example/blob/master/src/main.rs#L65
        self.surface.get_capabilities(&self.adapter).formats[0]
    }

    pub fn configure_surface(&mut self) {
        use egui_wgpu_backend::wgpu::CompositeAlphaMode;

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
