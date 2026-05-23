use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub content: String,
    pub ts: i64,
}

pub struct Buffer {
    pub messages: Vec<Message>,
}

impl Buffer {
    pub fn new(bytes: &[u8]) -> Self {
        let mut messages: Vec<Message> = std::str::from_utf8(bytes)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        messages.sort_by_key(|m| m.ts);
        Self { messages }
    }

    pub fn merge(base: &[u8], local: &[u8], remote: &[u8]) -> Vec<u8> {
        let base = Self::new(base);
        let mut local = Self::new(local);
        let remote = Self::new(remote);

        for msg in &remote.messages {
            if !base.messages.contains(msg) && !local.messages.contains(msg) {
                local.messages.push(msg.clone());
            }
        }

        local
            .messages
            .retain(|msg| !base.messages.contains(msg) || remote.messages.contains(msg));

        local.messages.sort_by_key(|m| m.ts);
        local.serialize()
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = self
            .messages
            .iter()
            .filter_map(|m| serde_json::to_string(m).ok())
            .collect::<Vec<_>>()
            .join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        out.into_bytes()
    }
}
