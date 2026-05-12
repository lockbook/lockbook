use egui_wgpu_renderer::{PreparedFrame, RendererState};
use workspace_rs::workspace::Workspace;

/// cbindgen:ignore
pub mod android;
pub mod apple;
pub mod response;

pub use response::Response;

#[repr(C)]
pub struct WgpuWorkspace<'window> {
    pub renderer: RendererState<'window>,
    pub workspace: Workspace,
    #[cfg(target_os = "android")]
    pub render_thread: Option<android::render_thread::RenderThread>,
}

impl WgpuWorkspace<'_> {
    pub fn prepare_frame(&mut self) -> (PreparedFrame, Response) {
        self.renderer.begin_frame();
        if cfg!(target_os = "android") || cfg!(target_os = "ios") {
            self.renderer
                .context
                .style_mut(|s| s.visuals.panel_fill = s.visuals.extreme_bg_color);
        }

        let keyboard_height =
            self.renderer.bottom_inset.unwrap_or(0) as f32 / self.renderer.screen.pixels_per_point;

        let workspace_frame =
            egui::Frame::default().fill(self.renderer.context.style().visuals.extreme_bg_color);

        let workspace_response = egui::CentralPanel::default()
            .frame(workspace_frame)
            .show(&self.renderer.context, |ui| {
                let mut rect = ui.max_rect();
                rect.max.y -= keyboard_height;
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    self.workspace.show(ui)
                })
                .inner
            })
            .inner;

        let prepared = self.renderer.prepare_frame();
        let response = Response::new(
            &self.renderer.context,
            &prepared.platform_output,
            &prepared.viewport_output,
            workspace_response,
        );

        (prepared, response)
    }

    pub fn render_prepared_frame(&mut self, prepared: PreparedFrame) {
        self.renderer.render_prepared_frame(prepared);
    }

    pub fn render_prepared_frame_offloaded(&mut self, prepared: PreparedFrame) {
        #[cfg(target_os = "android")]
        if let Some(render_thread) = &self.render_thread {
            render_thread.render(
                prepared,
                self.renderer.screen.size_in_pixels,
                self.renderer.screen.pixels_per_point,
            );

            self.renderer.shame_slow_frame();
            return;
        }

        self.renderer.render_prepared_frame(prepared);
    }

    pub fn frame(&mut self) -> Response {
        let (prepared, response) = self.prepare_frame();
        self.render_prepared_frame(prepared);
        response
    }

    pub fn frame_offloaded(&mut self) -> Response {
        let (prepared, response) = self.prepare_frame();
        self.render_prepared_frame_offloaded(prepared);
        response
    }
}
