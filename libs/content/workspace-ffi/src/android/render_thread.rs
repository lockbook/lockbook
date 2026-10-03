use egui_wgpu_renderer::wgpu::Surface;
use egui_wgpu_renderer::{PreparedFrame, RenderBackend};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

struct RenderRequest {
    prepared: PreparedFrame,
    size_in_pixels: [u32; 2],
    pixels_per_point: f32,
}

enum Message {
    Frame(RenderRequest),
    /// The view's window came back; frames go to it from now on.
    Surface(Surface<'static>),
}

pub struct RenderThread {
    tx: Option<Sender<Message>>,
    join_handle: Option<JoinHandle<()>>,
}

impl RenderThread {
    pub fn spawn(context: egui::Context, mut backend: RenderBackend<'static>) -> Self {
        let (tx, rx) = mpsc::channel();
        let join_handle = thread::spawn(move || run_render_loop(&context, &mut backend, rx));

        Self { tx: Some(tx), join_handle: Some(join_handle) }
    }

    pub fn render(&self, prepared: PreparedFrame, size_in_pixels: [u32; 2], pixels_per_point: f32) {
        let Some(tx) = &self.tx else { return };
        let request = RenderRequest { prepared, size_in_pixels, pixels_per_point };
        let _ = tx.send(Message::Frame(request));
    }

    pub fn replace_surface(&self, surface: Surface<'static>) {
        let Some(tx) = &self.tx else { return };
        let _ = tx.send(Message::Surface(surface));
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

/// Draws the latest frame waiting, with the textures of the frames it
/// skips, after any surface that arrived with them.
fn run_render_loop(
    context: &egui::Context, backend: &mut RenderBackend<'_>, rx: Receiver<Message>,
) {
    while let Ok(first) = rx.recv() {
        let mut latest: Option<RenderRequest> = None;
        for message in std::iter::once(first).chain(rx.try_iter()) {
            match message {
                Message::Surface(surface) => backend.replace_surface(surface),
                Message::Frame(mut next) => {
                    if let Some(mut skipped) = latest.take() {
                        let mut textures = std::mem::take(&mut skipped.prepared.textures_delta);
                        textures.append(std::mem::take(&mut next.prepared.textures_delta));
                        next.prepared.textures_delta = textures;
                    }
                    latest = Some(next);
                }
            }
        }
        let Some(latest) = latest else { continue };
        backend.render_prepared_frame(
            context,
            latest.prepared,
            latest.size_in_pixels,
            latest.pixels_per_point,
        );
    }
}
