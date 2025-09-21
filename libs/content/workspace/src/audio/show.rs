use egui::{Area, Frame, Id, Order, Pos2, Ui};
use egui_player::{player::Player};
use lb_rs::Uuid;
use tokio::runtime::Runtime;

pub struct Audio {
    pub id: Uuid,
    pub player: Player,
}

impl Audio {
    pub fn new(id: Uuid, bytes: Vec<u8>, model_path: String) -> Self {
        let mut player = Player::from_bytes(bytes);
        player.set_transcript_settings(egui_player::TranscriptionSettings::TranscriptLabel);
        player.set_model_download_path(model_path);

        Audio { id, player }
    }
    pub fn show(&mut self, ui: &mut Ui) {
        let rt = Runtime::new().unwrap();
        Area::new(Id::new(1))
            .order(Order::Background)
            .fixed_pos(Pos2 { x: ui.max_rect().min.x, y: 50.0 })
            .show(ui.ctx(), |ui| {
                Frame::none().show(ui, |ui| {
                    let _ = rt.block_on(async {
                        let _ = self.player.ui(ui);
                    });
                });
            });
    }
}
