use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use super::AccountUpdate;

const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(2);

pub struct SaveRequest {
    pub id: lb::Uuid,
    pub content: SaveRequestContent,
}

impl SaveRequest {
    pub const SHUTDOWN_REQ: Self =
        Self { id: lb::Uuid::nil(), content: SaveRequestContent::Text(String::new()) };
}

pub enum SaveRequestContent {
    Text(String),
    Draw(lb::Drawing),
}

impl super::AccountScreen {
    pub fn send_auto_save_signals(&self, ctx: &egui::Context) {
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || loop {
            thread::sleep(AUTO_SAVE_INTERVAL);
            update_tx.send(AccountUpdate::AutoSaveSignal).unwrap();
            ctx.request_repaint();
        });
    }

    pub fn process_save_requests(
        &mut self, ctx: &egui::Context, save_req_rx: mpsc::Receiver<SaveRequest>,
    ) {
        let update_tx = self.update_tx.clone();
        let core = self.core.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            while let Ok(req) = save_req_rx.recv() {
                if req.id == SaveRequest::SHUTDOWN_REQ.id {
                    update_tx.send(AccountUpdate::SaveRequestsDone).unwrap();
                    ctx.request_repaint();
                    return;
                }

                let result = match req.content {
                    SaveRequestContent::Text(s) => core.write_document(req.id, s.as_bytes()),
                    SaveRequestContent::Draw(d) => core.save_drawing(req.id, &d),
                }
                .map(|_| Instant::now());

                update_tx
                    .send(AccountUpdate::SaveResult(req.id, result))
                    .unwrap();
                ctx.request_repaint();
            }
        });
    }
}
