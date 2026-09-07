//! Chat tab — stub. The previous implementation was removed so this surface
//! can be rebuilt; workspace/FFI still open `.chat` files as this tab.

use std::sync::{Arc, RwLock};

use egui::{Rect, Ui};
use lb_rs::Uuid;
use lb_rs::model::account::Account;
use lb_rs::model::file_metadata::DocumentHmac;

use crate::file_cache::FileCache;
use crate::tab::markdown_editor::MdEdit;

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub seq: usize,
    pub initialized: bool,
    /// Placeholder for the iOS text-input bridge. `show` reports no field.
    composer: MdEdit,
    bytes: Vec<u8>,
}

impl Chat {
    pub fn new(
        bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, _account: Account, ctx: egui::Context,
        _files: Arc<RwLock<FileCache>>, _core: &lb_rs::blocking::Lb,
    ) -> Self {
        Self {
            id,
            hmac,
            seq: 0,
            initialized: false,
            composer: MdEdit::empty(ctx),
            bytes: bytes.to_vec(),
        }
    }

    pub fn focused_field(&mut self) -> &mut MdEdit {
        &mut self.composer
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        self.bytes = bytes.to_vec();
        self.hmac = hmac;
        self.seq += 1;
    }

    pub fn saved(&mut self, hmac: DocumentHmac, content: Vec<u8>) {
        self.hmac = Some(hmac);
        self.bytes = content;
    }

    pub fn kick_config_load(&mut self) {}

    pub fn will_consume_touch(&self, _pos: egui::Pos2) -> bool {
        false
    }

    pub fn show(&mut self, _ui: &mut Ui) -> (bool, Rect, bool, bool) {
        (false, Rect::NOTHING, false, false)
    }
}
