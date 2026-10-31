//! Previews for hosts: a tab rendered to PNG bytes. The host keeps the
//! images and judges staleness by the document's hmac; nothing here stores
//! them.

use egui::Vec2;
use workspace_rs::preview::{Headless, Look, PreviewGpu, PreviewRequest, encode_png};
use workspace_rs::tab::{Destination, Place, SessionId, TabResources};
use workspace_rs::theme::palette_v2::Theme;

use crate::WgpuWorkspace;

/// Room around a preview's content, in points.
const PADDING: f32 = 12.0;

/// Where a preview's frame is rendered.
enum Gpu {
    /// On the calling thread, with the live device.
    Direct(PreviewGpu),
    /// On Android's render thread, which owns the device.
    #[cfg(target_os = "android")]
    Thread(crate::android::render_thread::PreviewSender),
}

/// What a preview needs from the workspace, taken on the main thread; it
/// renders on any.
pub struct PreviewJob {
    res: TabResources,
    look: Look,
    req: PreviewRequest,
    gpu: Gpu,
}

impl WgpuWorkspace<'_> {
    /// A preview of open session `id` where it's parked, drawn in `theme`;
    /// None for one that isn't open or is a search, which draws in the
    /// workspace.
    pub fn preview_session(
        &self, id: SessionId, size: Vec2, scale: f32, theme: Theme,
    ) -> Option<PreviewJob> {
        let session = self.workspace.tab_strip.iter().find(|s| s.id == id)?;
        if matches!(session.dest, Destination::Search(_)) {
            return None;
        }
        Some(self.preview(session.place(), size, scale, theme))
    }

    /// A preview of `place`, a pinned document at a fragment, drawn in
    /// `theme`.
    pub fn preview(&self, place: Place, size: Vec2, scale: f32, theme: Theme) -> PreviewJob {
        let font_system = workspace_rs::register_font_system(&self.renderer.context);
        let look = Look { theme, ..Look::of(&self.renderer.context) };
        let req = PreviewRequest { place, size, scale, padding: PADDING, text: None };
        #[cfg(target_os = "android")]
        if let Some(thread) = &self.render_thread {
            return PreviewJob {
                res: self.workspace.resources(),
                look,
                req,
                gpu: Gpu::Thread(thread.preview_sender(font_system)),
            };
        }
        let backend = self.renderer.backend();
        PreviewJob {
            res: self.workspace.resources(),
            look,
            req,
            gpu: Gpu::Direct(PreviewGpu {
                device: backend.device.clone(),
                queue: backend.queue.clone(),
                font_system,
            }),
        }
    }
}

impl PreviewJob {
    /// Draws the preview until its tab settles and encodes it as a PNG.
    pub fn run(self) -> Result<Vec<u8>, String> {
        let frame = Headless::open(&self.res, self.look, &self.req).settled_frame()?;
        let (pixels, size) = match self.gpu {
            Gpu::Direct(gpu) => gpu.render(frame)?,
            #[cfg(target_os = "android")]
            Gpu::Thread(sender) => sender.render(frame)?,
        };
        encode_png(pixels, size)
    }
}
