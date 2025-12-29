use egui_wgpu::wgpu::{self, TextureDescriptor, TextureUsages};
use std::iter;
use std::time::Instant;
use workspace_rs::workspace::Workspace;

/// cbindgen:ignore
pub mod android;
pub mod apple;
pub mod response;

pub use response::Response;

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

    pub renderer: egui_wgpu::Renderer,
    pub screen: egui_wgpu::ScreenDescriptor,

    pub context: egui::Context,
    pub raw_input: egui::RawInput,

    pub workspace: Workspace,
}

impl WgpuWorkspace<'_> {
    pub fn frame(&mut self) -> Response {
        self.configure_surface(); // todo: does this need to happen?
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                eprintln!("wgpu::SurfaceError::Outdated");
                return Default::default(); // todo: could this be the source of a bug if some
                // response has a default value of true or something
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {e}");
                return Default::default();
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let msaa_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("msaa_texture"),
            size: output_frame.texture.size(),
            mip_level_count: output_frame.texture.mip_level_count(),
            sample_count: 4,
            dimension: output_frame.texture.dimension(),
            format: output_frame.texture.format(),
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // can probably use run
        self.set_egui_screen();
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.context.begin_frame(self.raw_input.take());

        if cfg!(target_os = "android") || cfg!(target_os = "ios") {
            self.context
                .style_mut(|s| s.visuals.panel_fill = s.visuals.extreme_bg_color);
        }
        let workspace_response = egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(self.context.style().visuals.panel_fill))
            .show(&self.context, |ui| self.workspace.show(ui))
            .inner;

        let full_output = self.context.end_frame();
        self.context.tessellation_options_mut(|w| {
            w.feathering = false;
        });

        let paint_jobs = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &self.screen,
        );

        // Record all render passes.
        {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), // todo: these are different
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    resolve_target: Some(&output_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.renderer.render(&mut pass, &paint_jobs, &self.screen);
        }

        // Submit the commands.
        self.queue.submit(iter::once(encoder.finish()));

        // Redraw egui
        output_frame.present();

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        Response::new(
            &self.context,
            full_output.platform_output,
            full_output.viewport_output,
            workspace_response,
        )
    }

    pub fn set_egui_screen(&mut self) {
        use egui::{Pos2, Rect};

        self.raw_input.screen_rect = Some(Rect {
            min: Pos2::ZERO,
            max: Pos2::new(
                self.screen.size_in_pixels[0] as f32 / self.screen.pixels_per_point,
                self.screen.size_in_pixels[1] as f32 / self.screen.pixels_per_point,
            ),
        });
        self.context
            .set_pixels_per_point(self.screen.pixels_per_point);
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        // todo: is this really fine?
        // from here: https://github.com/hasenbanck/egui_example/blob/master/src/main.rs#L65
        self.surface.get_capabilities(&self.adapter).formats[0]
    }

    pub fn configure_surface(&mut self) {
        use egui_wgpu::wgpu::CompositeAlphaMode;

        let resized = self.screen.size_in_pixels[0] != self.surface_width
            || self.screen.size_in_pixels[1] != self.surface_height;
        let visible = self.screen.size_in_pixels[0] * self.screen.size_in_pixels[1] != 0;
        if resized && visible {
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.surface_format(),
                width: self.screen.size_in_pixels[0],
                height: self.screen.size_in_pixels[1],
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: CompositeAlphaMode::Auto,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            self.surface.configure(&self.device, &surface_config);
            self.surface_width = self.screen.size_in_pixels[0];
            self.surface_height = self.screen.size_in_pixels[1];
        }
    }
}
