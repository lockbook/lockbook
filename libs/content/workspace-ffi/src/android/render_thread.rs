use egui_wgpu_renderer::{PreparedFrame, RenderBackend};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

struct RenderRequest {
    prepared: PreparedFrame,
    size_in_pixels: [u32; 2],
    pixels_per_point: f32,
}

pub struct RenderThread {
    tx: Option<Sender<RenderRequest>>,
    join_handle: Option<JoinHandle<()>>,
}

impl RenderThread {
    pub fn spawn(mut backend: RenderBackend<'static>) -> Self {
        let (tx, rx) = mpsc::channel();
        let join_handle = thread::spawn(move || run_render_loop(&mut backend, rx));

        Self { tx: Some(tx), join_handle: Some(join_handle) }
    }

    pub fn render(&self, prepared: PreparedFrame, size_in_pixels: [u32; 2], pixels_per_point: f32) {
        let Some(tx) = &self.tx else { return };
        let _ = tx.send(RenderRequest { prepared, size_in_pixels, pixels_per_point });
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

fn run_render_loop(backend: &mut RenderBackend<'_>, rx: Receiver<RenderRequest>) {
    while let Ok(mut request) = rx.recv() {
        while let Ok(next_request) = rx.try_recv() {
            request = next_request;
        }

        backend.render_prepared_frame(
            request.prepared,
            request.size_in_pixels,
            request.pixels_per_point,
        );
    }
}
