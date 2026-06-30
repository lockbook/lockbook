use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub content: String,
    pub ts: i64,
    /// Unique per message. `None` on messages written before ids existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    /// Sent by `from`'s agent rather than typed by them. Agent messages
    /// never trigger agent invocation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub agent: bool,
    /// A tool round-trip by `from`'s agent; `content` is a short human
    /// summary ("read_file /notes/todo.md") for rendering. The record makes
    /// the transcript the agent's complete memory: reopening a chat replays
    /// these into model context, so a restarted agent knows everything the
    /// live one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolRecord>,
    /// On agent replies: token usage of the turn that produced this message.
    /// Chat-lifetime usage is the fold of these over the transcript.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// A harness-level error ("rate limited", "network down") from `from`'s
    /// agent. Rendered as a dim red row; excluded from agent context (the
    /// model never saw it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub error: bool,
    /// A `from`-authored configuration entry rather than a chat message (no
    /// `content`). Carries this user's per-chat agent settings; the latest by
    /// `ts` wins. Excluded from rendering and from agent context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<ChatConfig>,
    /// Fields this client doesn't know about, preserved verbatim so a merge
    /// performed by an older client can't strip them.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Per-user, per-chat agent configuration, carried as a non-message entry in
/// the `.chat` log. Each user's latest entry (by `ts`) is their selection for
/// this chat; provider *credentials* stay device-local, never in the log.
#[derive(Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct ChatConfig {
    /// The model this user drives this chat with; absent → the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelSelection>,
}

/// A provider+model selection. `provider` names an entry in the device's
/// provider registry (`/chat.json`); `model` may be empty to mean the
/// provider's default.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ModelSelection {
    pub provider: String,
    pub model: String,
}

/// Token usage of the agent turn that produced a message.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ToolRecord {
    pub name: String,
    pub args: Value,
    /// The result text the model saw (possibly truncated by the writer).
    pub result: String,
}

impl Message {
    pub fn new(from: String, content: String, ts: i64) -> Self {
        Self {
            from,
            content,
            ts,
            id: Some(Uuid::new_v4()),
            agent: false,
            tool: None,
            usage: None,
            error: false,
            config: None,
            extra: Map::new(),
        }
    }

    /// A `from`-authored config entry — carries no chat content, isn't rendered,
    /// and never enters agent context.
    pub fn config_entry(from: String, ts: i64, config: ChatConfig) -> Self {
        let mut m = Self::new(from, String::new(), ts);
        m.config = Some(config);
        m
    }
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
        serde_json::to_string(&Message::new(from.into(), content.into(), ts)).unwrap() + "\n"
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

    /// A client that doesn't know a field must carry it through parse →
    /// merge → serialize untouched, or newer clients' data gets stripped.
    #[test]
    fn merge_preserves_unknown_fields() {
        let base = line("a", "hello", 1);
        let remote = base.clone()
            + "{\"from\":\"b\",\"content\":\"hi\",\"ts\":2,\"reply_to\":\"abc\",\"agent\":true}\n";

        let merged = Buffer::merge(base.as_bytes(), base.as_bytes(), remote.as_bytes());
        let merged = String::from_utf8(merged).unwrap();

        assert!(merged.contains("\"reply_to\":\"abc\""), "unknown field stripped: {merged}");
        assert!(merged.contains("\"agent\":true"));
    }
}
