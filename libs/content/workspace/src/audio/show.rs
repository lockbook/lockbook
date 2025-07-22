use egui::{Area, Frame, Id, Order, Pos2, Ui};
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
        Area::new(Id::new(1))
            .order(Order::Background)
            .fixed_pos(Pos2 { x: ui.max_rect().min.x, y: 50.0 })
            .show(ui.ctx(), |ui| {
                Frame::none().show(ui, |ui| {
                    let _ = self.player.ui(ui);
                });
            });
    }
}
