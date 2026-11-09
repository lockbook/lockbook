//! Lockbook desktop — custom host + product shell.

// Paint/show helpers take many injected deps (tokens, queue, expanded, …).
#![allow(clippy::too_many_arguments)]
// Session / modal payloads dwarf unit variants; boxing would muddy call sites.
#![allow(clippy::large_enum_variant)]

mod components;
pub mod host;
mod link_inbox;
mod perf;
mod settings;
pub mod shell;
mod util;

pub use crate::settings::Settings;
pub use crate::shell::ShellApp;
pub use host::run;

use egui::{OrderedViewportIdMap, PlatformOutput, ViewportOutput};
use egui_wgpu_renderer::{PreparedFrame, RendererState};
use tracing::instrument;

/// Custom-host product frame: wgpu surface + shell.
pub struct WgpuLockbook<'window> {
    pub renderer: RendererState<'window>,
    pub queued_events: Vec<egui::Event>,
    pub double_queued_events: Vec<egui::Event>,
    pub app: ShellApp,
}

#[derive(Default)]
pub struct Output {
    pub platform: PlatformOutput,
    pub viewport: OrderedViewportIdMap<ViewportOutput>,
}

impl WgpuLockbook<'_> {
    #[instrument(level = "trace", skip_all)]
    pub fn frame(&mut self) -> Output {
        let _sample = lb::service::perf::Sample::new();
        let prepared = self.frame_ui();
        let platform = prepared.platform_output.clone();
        let viewport = prepared.viewport_output.clone();
        self.frame_gpu(prepared);

        self.renderer
            .raw_input
            .events
            .append(&mut self.queued_events);
        self.queued_events.append(&mut self.double_queued_events);
        if !self.renderer.raw_input.events.is_empty() {
            self.renderer.context.request_repaint();
        }

        Output { platform, viewport }
    }

    #[instrument(level = "trace", skip_all)]
    fn frame_ui(&mut self) -> PreparedFrame {
        self.renderer.begin_frame();
        self.app.ui(&self.renderer.context);
        let enforced = lb::service::budget::ENFORCED && self.app.is_dev();
        self.renderer
            .set_frame_budget(enforced.then_some(lb::service::budget::FRAME_BUDGET));
        self.renderer.prepare_frame()
    }

    #[instrument(level = "trace", skip_all)]
    fn frame_gpu(&mut self, prepared: PreparedFrame) {
        self.renderer.render_prepared_frame(prepared);
    }
}
