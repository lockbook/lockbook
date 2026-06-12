//! Chat/agent settings, stored as `/chat.json` in the user's lockbook.
//! Lockbook documents are encrypted and synced, so the API key set up on one
//! device follows the user to every other device.

use lb_rs::blocking::Lb;
use serde::{Deserialize, Serialize};

use super::harness::DEFAULT_MODEL;

/// Parsed `/chat.json`. Every field is optional so a partial file works and
/// new fields can land without breaking older clients; unknown fields are
/// ignored (and preserved on disk, since we never write this file).
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ChatSettings {
    pub api_key: Option<String>,
    pub model: Option<String>,
}

impl ChatSettings {
    /// Read and parse `/chat.json`; defaults when missing or malformed.
    pub fn load(core: &Lb) -> Self {
        let Ok(file) = core.get_by_path("/chat.json") else { return Self::default() };
        let Ok(bytes) = core.read_document(file.id, false) else { return Self::default() };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    /// The API key to use: `/chat.json`, falling back to the
    /// `ANTHROPIC_API_KEY` env var.
    pub fn api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
    }

    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or(DEFAULT_MODEL)
    }
}
