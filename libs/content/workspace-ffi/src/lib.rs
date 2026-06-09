use egui_wgpu_renderer::{PreparedFrame, RendererState};
use workspace_rs::tab::{ContentState, TabContent};
use workspace_rs::workspace::Workspace;

/// cbindgen:ignore
pub mod android;
pub mod apple;
pub mod response;

pub use response::Response;

pub fn current_tab_type(ws: &Workspace) -> i32 {
    match ws.current_tab() {
        None => 0,
        Some(tab) => match &tab.content {
            ContentState::Open(content) => match content {
                TabContent::Image(_) => 2,
                TabContent::Markdown(_) => 3,
                TabContent::Pdf(_) => 5,
                TabContent::Svg(_) => 6,
                #[cfg(not(target_family = "wasm"))]
                TabContent::MindMap(_) => 7,
                TabContent::SpaceInspector(_) => 8,
                TabContent::Chat(_) => 9,
                TabContent::Search(_) => 0,
            },
            _ => 1,
        },
    }
}

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

        // Surface the soft-keyboard height (>0 when shown) so tabs can size
        // fixed bottom UI — the chat composer drops its nav-bar padding while
        // the keyboard is up.
        self.renderer
            .context
            .memory_mut(|m| m.data.insert_temp(egui::Id::new("ws_keyboard_height"), keyboard_height));

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
