//! Lockbook desktop — custom host + product shell.

// Paint/show helpers take many injected deps (tokens, queue, expanded, …).
#![allow(clippy::too_many_arguments)]
// Session / modal payloads dwarf unit variants; boxing would muddy call sites.
#![allow(clippy::large_enum_variant)]

mod components;
pub mod host;
mod settings;
pub mod shell;
mod util;

pub use crate::settings::Settings;
pub use crate::shell::ShellApp;
pub use host::run;

pub const DEV_USERS: &[&str] = &["parth", "adam", "travis", "at"];

use egui::{PlatformOutput, ViewportIdMap, ViewportOutput};
use egui_wgpu_renderer::RendererState;

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
    pub viewport: ViewportIdMap<ViewportOutput>,
}

impl WgpuLockbook<'_> {
    pub fn frame(&mut self) -> Output {
        self.renderer.begin_frame();
        self.app.ui(&self.renderer.context);
        self.renderer.set_is_dev(self.app.is_dev());
        let (platform, viewport) = self.renderer.end_frame();

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
}
