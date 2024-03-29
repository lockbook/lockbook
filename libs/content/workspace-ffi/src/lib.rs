use egui::{Pos2, Rect};

use egui_wgpu_backend::wgpu;
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use lb_external_interface::lb_rs::Uuid;
use std::time::Instant;

use std::iter;

mod cursor_icon;

use workspace_rs::workspace::Workspace;

#[cfg(target_vendor = "apple")]
pub mod apple;

// #[cfg(target_os = "android")]
pub mod android;

#[cfg(target_vendor = "apple")]
pub use apple::resp::*;

#[cfg(target_os = "android")]
pub use android::resp::*;

#[repr(C)]
pub struct WgpuWorkspace {
    pub start_time: Instant,

    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
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

impl WgpuWorkspace {
    // merge functions after everything is well and good
    #[cfg(target_os = "android")]
    pub fn frame(&mut self) -> IntegrationOutput {
        let mut out = IntegrationOutput::default();

        self.configure_surface();
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                eprintln!("wgpu::SurfaceError::Outdated");
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

        out.workspace_resp = self.workspace.draw(&self.context).into();

        let full_output = self.context.end_frame();

        if !full_output.platform_output.copied_text.is_empty() {
            out.copied_text = full_output.platform_output.copied_text;
        }

        if let Some(url) = full_output.platform_output.open_url {
            out.url_opened = url.url;
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

        out.redraw_in = full_output.repaint_after.as_millis();
        out
    }

    #[cfg(target_vendor = "apple")]
    pub fn frame(&mut self) -> IntegrationOutput {
        use std::ffi::CString;

        let mut out = IntegrationOutput::default();
        self.configure_surface();
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                eprintln!("wgpu::SurfaceError::Outdated");
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

        out.workspace_resp = self.workspace.draw(&self.context).into();

        let full_output = self.context.end_frame();

        #[cfg(target_os = "ios")]
        {
            if let Some(markdown) = self.workspace.current_tab_markdown() {
                out.workspace_resp.text_updated = markdown.editor.text_updated;
                out.workspace_resp.selection_updated = (markdown.editor.scroll_area_offset
                    != markdown.editor.old_scroll_area_offset)
                    || markdown.editor.selection_updated;
            }
        }

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
            };
            self.surface.configure(&self.device, &surface_config);
            self.surface_width = self.screen.physical_width;
            self.surface_height = self.screen.physical_height;
        }
    }
}
#[repr(C)]
#[derive(Debug, Default)]
pub struct CUuid([u8; 16]);

impl From<Uuid> for CUuid {
    fn from(value: Uuid) -> Self {
        Self(value.into_bytes())
    }
}

impl From<CUuid> for Uuid {
    fn from(value: CUuid) -> Self {
        Uuid::from_bytes(value.0)
    }
}
