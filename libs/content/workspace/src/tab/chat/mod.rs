//! Placeholder chat tab while the chat is rebuilt. Shows the entry count and
//! keeps the document bytes intact through load, save, and reload.

use egui::{Rect, Ui};
use lb_rs::Uuid;
use lb_rs::model::chat::Chat as Transcript;
use lb_rs::model::file_metadata::DocumentHmac;

use crate::tab::markdown_editor::MdEdit;

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub seq: usize,
    pub initialized: bool,
    bytes: Vec<u8>,
}

impl Chat {
    pub fn new(bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>) -> Self {
        Self { id, hmac, seq: 0, initialized: false, bytes: bytes.to_vec() }
    }

    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        self.bytes = bytes.to_vec();
        self.hmac = hmac;
    }

    pub fn saved(&mut self, hmac: DocumentHmac, _content: Vec<u8>) {
        self.hmac = Some(hmac);
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn show(&mut self, ui: &mut Ui) -> (bool, Rect, bool, bool) {
        let entries = Transcript::parse(&self.bytes).entries.len();
        ui.centered_and_justified(|ui| {
            ui.label(format!("Chat is being rebuilt. {entries} entries."));
        });
        (false, Rect::NOTHING, false, false)
    }

    pub fn focused_field(&mut self) -> Option<&mut MdEdit> {
        None
    }

    pub fn will_consume_touch(&self, _pos: egui::Pos2) -> bool {
        false
    }

    pub fn kick_config_load(&mut self) {}
}
