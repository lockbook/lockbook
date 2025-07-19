use egui::Ui;
use egui_player::{player::Player, InputMode};
use lb_rs::Uuid;

pub struct Audio {
    pub id: Uuid,
    pub player: Player,
}

impl Audio {
    pub fn new(id: Uuid, bytes: Vec<u8>) -> Self {
        let mut player = Player::new(InputMode::Bytes(bytes));
        player.set_transcript_settings(egui_player::TranscriptionSettings::TranscriptLabel);

        Audio { id, player }
    }
    pub fn show(&mut self, ui: &mut Ui) {
        self.player.ui(ui);
    }
}
