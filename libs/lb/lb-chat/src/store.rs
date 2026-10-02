//! Where the `.chat` bytes live. Writes are compare-and-swap on the hmac the
//! bytes were read with; a conflict means re-read and re-apply.

use std::sync::{Arc, Mutex};

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Chat, Entry};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_metadata::DocumentHmac;

pub trait Store: Send {
    fn load(&self) -> Result<(Option<DocumentHmac>, Vec<u8>), String>;
    /// `Err(None)` is a compare-and-swap conflict; load and try again.
    fn save(
        &self, hmac: Option<DocumentHmac>, bytes: Vec<u8>,
    ) -> Result<DocumentHmac, Option<String>>;

    /// Reads the chat, applies `edit`, and writes it back, retrying on conflict.
    fn update(&self, edit: &mut dyn FnMut(&mut Chat)) -> Result<Chat, String> {
        for _ in 0..8 {
            let (hmac, bytes) = self.load()?;
            let mut chat = Chat::parse(&bytes);
            edit(&mut chat);
            match self.save(hmac, chat.serialize()) {
                Ok(_) => return Ok(chat),
                Err(Some(err)) => return Err(err),
                Err(None) => continue,
            }
        }
        Err("the chat kept changing underneath the write".into())
    }

    /// Appends `entry` and returns it as stored.
    fn append(&self, entry: Entry) -> Result<Entry, String> {
        let chat = self.update(&mut |chat| chat.push(entry.clone()))?;
        Ok(chat.entries.last().cloned().unwrap_or(entry))
    }
}

pub struct LbStore {
    pub lb: Lb,
    pub id: Uuid,
}

impl Store for LbStore {
    fn load(&self) -> Result<(Option<DocumentHmac>, Vec<u8>), String> {
        self.lb
            .read_document_with_hmac(self.id, false)
            .map_err(|e| e.to_string())
    }

    fn save(
        &self, hmac: Option<DocumentHmac>, bytes: Vec<u8>,
    ) -> Result<DocumentHmac, Option<String>> {
        self.lb
            .safe_write(self.id, hmac, bytes, None)
            .map_err(|e| match e.kind {
                LbErrKind::ReReadRequired => None,
                _ => Some(e.to_string()),
            })
    }
}

/// In-memory store for tests and for callers that persist elsewhere.
#[derive(Clone, Default)]
pub struct MemStore {
    state: Arc<Mutex<(u8, Vec<u8>)>>,
}

impl MemStore {
    pub fn bytes(&self) -> Vec<u8> {
        self.state.lock().unwrap().1.clone()
    }

    pub fn chat(&self) -> Chat {
        Chat::parse(&self.bytes())
    }
}

impl Store for MemStore {
    fn load(&self) -> Result<(Option<DocumentHmac>, Vec<u8>), String> {
        let state = self.state.lock().unwrap();
        Ok((Some([state.0; 32]), state.1.clone()))
    }

    fn save(
        &self, hmac: Option<DocumentHmac>, bytes: Vec<u8>,
    ) -> Result<DocumentHmac, Option<String>> {
        let mut state = self.state.lock().unwrap();
        if hmac != Some([state.0; 32]) {
            return Err(None);
        }
        state.0 = state.0.wrapping_add(1);
        state.1 = bytes;
        Ok([state.0; 32])
    }
}
