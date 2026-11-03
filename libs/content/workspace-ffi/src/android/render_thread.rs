use egui_wgpu_renderer::{PreparedFrame, RenderBackend};
use glyphon::FontSystem;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use workspace_rs::preview::{PreviewFrame, PreviewGpu};

enum Request {
    Frame {
        prepared: PreparedFrame,
        size_in_pixels: [u32; 2],
        pixels_per_point: f32,
    },
    /// A preview, rendered on the device the thread owns and sent back.
    Preview {
        frame: PreviewFrame,
        font_system: Arc<Mutex<FontSystem>>,
        reply: Sender<Result<(Vec<u8>, [u32; 2]), String>>,
    },
}

pub struct RenderThread {
    tx: Option<Sender<Request>>,
    join_handle: Option<JoinHandle<()>>,
}

/// Sends a preview's frame to the render thread and waits for its pixels.
pub struct PreviewSender {
    tx: Sender<Request>,
    font_system: Arc<Mutex<FontSystem>>,
}

impl PreviewSender {
    pub fn render(self, frame: PreviewFrame) -> Result<(Vec<u8>, [u32; 2]), String> {
        let (reply, rx) = mpsc::channel();
        let request = Request::Preview { frame, font_system: self.font_system, reply };
        self.tx.send(request).map_err(|_| "render thread stopped")?;
        rx.recv().map_err(|_| "render thread stopped".to_string())?
    }
}

impl RenderThread {
    pub fn spawn(context: egui::Context, mut backend: RenderBackend<'static>) -> Self {
        let (tx, rx) = mpsc::channel();
        let join_handle = thread::spawn(move || run_render_loop(&context, &mut backend, rx));

        Self { tx: Some(tx), join_handle: Some(join_handle) }
    }

    pub fn render(&self, prepared: PreparedFrame, size_in_pixels: [u32; 2], pixels_per_point: f32) {
        let Some(tx) = &self.tx else { return };
        let _ = tx.send(Request::Frame { prepared, size_in_pixels, pixels_per_point });
    }

    pub fn preview_sender(&self, font_system: Arc<Mutex<FontSystem>>) -> PreviewSender {
        let tx = self.tx.clone().expect("render thread stopped");
        PreviewSender { tx, font_system }
    }
}

impl Drop for RenderThread {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

/// Renders the latest of the surface frames waiting, with every waiting
/// frame's texture changes, and each preview in turn.
fn run_render_loop(
    context: &egui::Context, backend: &mut RenderBackend<'_>, rx: Receiver<Request>,
) {
    while let Ok(first) = rx.recv() {
        let mut frame: Option<(PreparedFrame, [u32; 2], f32)> = None;
        for request in std::iter::once(first).chain(rx.try_iter()) {
            match request {
                Request::Frame { mut prepared, size_in_pixels, pixels_per_point } => {
                    if let Some((mut older, _, _)) = frame.take() {
                        let mut textures = std::mem::take(&mut older.textures_delta);
                        textures.append(std::mem::take(&mut prepared.textures_delta));
                        prepared.textures_delta = textures;
                    }
                    frame = Some((prepared, size_in_pixels, pixels_per_point));
                }
                Request::Preview { frame: preview, font_system, reply } => {
                    let gpu = PreviewGpu {
                        device: backend.device.clone(),
                        queue: backend.queue.clone(),
                        font_system,
                    };
                    let _ = reply.send(gpu.render(preview));
                }
            }
        }
        if let Some((prepared, size_in_pixels, pixels_per_point)) = frame {
            backend.render_prepared_frame(context, prepared, size_in_pixels, pixels_per_point);
        }
    }
}
