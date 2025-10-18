use egui::{Area, Frame, Id, Order, Pos2, Ui};
use egui_player::player::Player;
use lb_rs::Uuid;
use tokio::runtime::Runtime;

pub struct Audio {
    pub id: Uuid,
    pub player: Player,
    pub rt: Runtime,
}

impl Audio {
    pub fn new(id: Uuid, bytes: Vec<u8>, model_path: String) -> Self {
        let mut player = Player::from_bytes(bytes);
        player.set_transcript_settings(egui_player::TranscriptionSettings::TranscriptLabel);
        player.set_model_download_path(model_path);

        Audio { id, player, rt: Runtime::new().unwrap() }
    }
    pub fn show(&mut self, ui: &mut Ui) {
        Area::new(Id::new(1))
            .order(Order::Background)
            .fixed_pos(Pos2 { x: ui.max_rect().min.x, y: 50.0 })
            .show(ui.ctx(), |ui| {
                Frame::none().show(ui, |ui| {
                    self.rt.block_on(async {
                        self.player.ui(ui);
                    });
                });
            });
    }
}
