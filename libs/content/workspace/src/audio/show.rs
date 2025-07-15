use egui;
use egui_player::player::Player;
use lb_rs::Uuid;

pub struct Audio {
    pub id: Uuid,
    pub player: Player,
}

impl Audio {
    pub fn new(id: Uuid, file_path: String) -> Self {
        let mut player = Player::new(&file_path);
        player.set_transcript_settings(egui_player::TranscriptionSettings::TranscriptLabel);

        Audio { id, player }
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.player.ui(ui);
    }
}
