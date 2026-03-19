use egui_wgpu_renderer::RendererState;
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
}

impl WgpuWorkspace<'_> {
    pub fn frame(&mut self) -> Response {
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

        let (platform, viewport) = self.renderer.end_frame();

        Response::new(&self.renderer.context, platform, viewport, workspace_response)
    }
}
