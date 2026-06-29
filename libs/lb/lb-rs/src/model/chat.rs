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

#[cfg(test)]
mod tests {
    use super::*;

    fn line(from: &str, content: &str, ts: i64) -> String {
        serde_json::to_string(&Message { from: from.into(), content: content.into(), ts }).unwrap()
            + "\n"
    }

    /// Concurrent appends on a shared base union to all turns, each once,
    /// ordered by ts — the case the sync engine's `Chat` arm relies on.
    #[test]
    fn merge_unions_concurrent_appends() {
        let base = line("a", "hello", 1);
        let local = base.clone() + &line("a", "one", 2);
        let remote = base.clone() + &line("b", "two", 3);

        let merged =
            Buffer::new(&Buffer::merge(base.as_bytes(), local.as_bytes(), remote.as_bytes()))
                .messages;

        let contents: Vec<_> = merged.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, ["hello", "one", "two"]);
    }
}
