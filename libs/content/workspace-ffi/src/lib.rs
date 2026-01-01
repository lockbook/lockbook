use egui_wgpu::wgpu::{self, TextureDescriptor, TextureUsages};
use egui_wgpu_renderer::RendererState;
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
    pub renderer: RendererState<'window>,
    pub start_time: Instant,
    pub workspace: Workspace,
}

impl WgpuWorkspace<'_> {
    pub fn frame(&mut self) -> Response {
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
}
