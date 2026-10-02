//! The `.chat` document: one JSON entry per line, append-only, merged across
//! devices as a set union by id. Only settled facts are written (a message
//! that was sent, a reply that finished, a tool call that returned), so
//! nothing in flight is ever reconciled.

use std::collections::HashSet;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use uuid::Uuid;
use web_time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: Uuid,
    /// Milliseconds since the epoch at commit time. Entries order by `(ts, id)`.
    pub ts: i64,
    /// Username whose agent produced the entry, or who typed it.
    pub from: String,
    pub body: Body,
    /// Fields this client doesn't know, carried verbatim through merge and save.
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    User {
        text: String,
        spoken: bool,
        mentions: Vec<Mention>,
    },
    Assistant {
        text: String,
        model: String,
        usage: Usage,
        spoken: bool,
        interrupted: bool,
    },
    Tool {
        name: String,
        args: Value,
        result: String,
        ok: bool,
        server: bool,
    },
    /// A harness failure the model never saw.
    Error {
        text: String,
    },
    Settings(Settings),
    /// A kind this client doesn't know; its fields live in [`Entry::extra`].
    Other(String),
}

/// A file referenced by a message, never its bytes. `id` survives a move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mention {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
}

/// Per-user chat settings; the latest entry by `(ts, id)` wins wholesale.
/// The folder the chat lives in is always included and never listed.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Settings {
    /// `provider/model`; absent means the default provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

/// Disjoint token counts; total context is their sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

impl Entry {
    fn new(from: impl Into<String>, body: Body) -> Self {
        Self { id: Uuid::new_v4(), ts: now_ms(), from: from.into(), body, extra: Map::new() }
    }

    pub fn user(from: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(from, Body::User { text: text.into(), spoken: false, mentions: Vec::new() })
    }

    pub fn assistant(
        from: impl Into<String>, text: impl Into<String>, model: impl Into<String>, usage: Usage,
    ) -> Self {
        let body = Body::Assistant {
            text: text.into(),
            model: model.into(),
            usage,
            spoken: false,
            interrupted: false,
        };
        Self::new(from, body)
    }

    pub fn tool(
        from: impl Into<String>, name: impl Into<String>, args: Value, result: impl Into<String>,
        ok: bool,
    ) -> Self {
        let body = Body::Tool { name: name.into(), args, result: result.into(), ok, server: false };
        Self::new(from, body)
    }

    pub fn error(from: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(from, Body::Error { text: text.into() })
    }

    pub fn settings(from: impl Into<String>, settings: Settings) -> Self {
        Self::new(from, Body::Settings(settings))
    }

    /// The text a reader sees; empty for settings and unknown kinds.
    pub fn text(&self) -> &str {
        match &self.body {
            Body::User { text, .. } | Body::Assistant { text, .. } | Body::Error { text } => text,
            Body::Tool { result, .. } => result,
            Body::Settings(_) | Body::Other(_) => "",
        }
    }

    fn from_map(mut map: Map<String, Value>) -> Option<Self> {
        let id = Uuid::parse_str(map.remove("id")?.as_str()?).ok()?;
        let ts = map
            .remove("ts")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let from = take_string(&mut map, "from");
        let kind = take_string(&mut map, "kind");
        let body = match kind.as_str() {
            "user" => Body::User {
                text: take_string(&mut map, "text"),
                spoken: take_bool(&mut map, "spoken", false),
                mentions: take(&mut map, "mentions").unwrap_or_default(),
            },
            "assistant" => Body::Assistant {
                text: take_string(&mut map, "text"),
                model: take_string(&mut map, "model"),
                usage: take(&mut map, "usage").unwrap_or_default(),
                spoken: take_bool(&mut map, "spoken", false),
                interrupted: take_bool(&mut map, "interrupted", false),
            },
            "tool" => Body::Tool {
                name: take_string(&mut map, "name"),
                args: map.remove("args").unwrap_or(Value::Null),
                result: take_string(&mut map, "result"),
                ok: take_bool(&mut map, "ok", true),
                server: take_bool(&mut map, "server", false),
            },
            "error" => Body::Error { text: take_string(&mut map, "text") },
            "settings" => Body::Settings(Settings {
                model: take(&mut map, "model"),
                include: take(&mut map, "include").unwrap_or_default(),
                exclude: take(&mut map, "exclude").unwrap_or_default(),
            }),
            other => Body::Other(other.to_string()),
        };
        Some(Self { id, ts, from, body, extra: map })
    }

    fn to_map(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("id".into(), json!(self.id));
        m.insert("ts".into(), json!(self.ts));
        m.insert("from".into(), json!(self.from));
        let mut put = |k: &str, v: Value| {
            m.insert(k.into(), v);
        };
        match &self.body {
            Body::User { text, spoken, mentions } => {
                put("kind", json!("user"));
                put("text", json!(text));
                if *spoken {
                    put("spoken", json!(true));
                }
                if !mentions.is_empty() {
                    put("mentions", json!(mentions));
                }
            }
            Body::Assistant { text, model, usage, spoken, interrupted } => {
                put("kind", json!("assistant"));
                put("text", json!(text));
                put("model", json!(model));
                put("usage", json!(usage));
                if *spoken {
                    put("spoken", json!(true));
                }
                if *interrupted {
                    put("interrupted", json!(true));
                }
            }
            Body::Tool { name, args, result, ok, server } => {
                put("kind", json!("tool"));
                put("name", json!(name));
                put("args", args.clone());
                put("result", json!(result));
                put("ok", json!(ok));
                if *server {
                    put("server", json!(true));
                }
            }
            Body::Error { text } => {
                put("kind", json!("error"));
                put("text", json!(text));
            }
            Body::Settings(settings) => {
                put("kind", json!("settings"));
                if let Value::Object(fields) = json!(settings) {
                    for (k, v) in fields {
                        put(&k, v);
                    }
                }
            }
            Body::Other(kind) => put("kind", json!(kind)),
        }
        for (k, v) in &self.extra {
            m.entry(k.clone()).or_insert_with(|| v.clone());
        }
        m
    }

    pub fn to_json_line(&self) -> String {
        serde_json::to_string(&Value::Object(self.to_map())).expect("maps serialize")
    }
}

fn take<T: DeserializeOwned>(map: &mut Map<String, Value>, key: &str) -> Option<T> {
    map.remove(key).and_then(|v| serde_json::from_value(v).ok())
}

fn take_string(map: &mut Map<String, Value>, key: &str) -> String {
    map.remove(key)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn take_bool(map: &mut Map<String, Value>, key: &str, default: bool) -> bool {
    map.remove(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chat {
    /// Sorted by `(ts, id)`, ids unique.
    pub entries: Vec<Entry>,
}

impl Chat {
    /// Lines that aren't a JSON object with an id are dropped; everything else
    /// is kept, known kind or not.
    pub fn parse(bytes: &[u8]) -> Self {
        let text = String::from_utf8_lossy(bytes);
        let mut seen = HashSet::new();
        let mut entries: Vec<Entry> = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter_map(|value| match value {
                Value::Object(map) => Entry::from_map(map),
                _ => None,
            })
            .filter(|entry| seen.insert(entry.id))
            .collect();
        sort(&mut entries);
        Self { entries }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = String::new();
        for entry in &self.entries {
            out.push_str(&entry.to_json_line());
            out.push('\n');
        }
        out.into_bytes()
    }

    /// Appends at the end: a timestamp that doesn't exceed the last entry's is
    /// bumped, so commit order within one writer is file order.
    pub fn push(&mut self, mut entry: Entry) {
        if let Some(last) = self.entries.last() {
            entry.ts = entry.ts.max(last.ts + 1);
        }
        self.entries.push(entry);
    }

    /// Removes `id` and everything after it. Returns whether `id` was present.
    pub fn truncate_from(&mut self, id: Uuid) -> bool {
        match self.entries.iter().position(|e| e.id == id) {
            Some(i) => {
                self.entries.truncate(i);
                true
            }
            None => false,
        }
    }

    pub fn settings_for(&self, user: &str) -> Settings {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.from == user)
            .find_map(|e| match &e.body {
                Body::Settings(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// What content search indexes: what people and models said.
    pub fn text(&self) -> String {
        self.entries
            .iter()
            .filter(|e| matches!(e.body, Body::User { .. } | Body::Assistant { .. }))
            .map(|e| e.text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            let when = chrono::DateTime::from_timestamp_millis(e.ts)
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default();
            match &e.body {
                Body::User { text, .. } => {
                    out.push_str(&format!("**{}** · {when}\n\n{text}\n\n", e.from))
                }
                Body::Assistant { text, model, .. } => {
                    out.push_str(&format!("**{model}** · {when}\n\n{text}\n\n"))
                }
                Body::Tool { name, args, ok, .. } => {
                    let status = if *ok { "ok" } else { "failed" };
                    out.push_str(&format!("- `{name}` {args} · {status}\n\n"));
                }
                Body::Error { text } => out.push_str(&format!("> {text}\n\n")),
                Body::Settings(_) | Body::Other(_) => {}
            }
        }
        out
    }
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by_key(|e| (e.ts, e.id));
}

/// Three-way merge of two edits of `base`. Union by id; an entry one side
/// deleted from `base` stays deleted; identical ids take the local copy.
pub fn merge(base: &[u8], local: &[u8], remote: &[u8]) -> Vec<u8> {
    let base = Chat::parse(base);
    let local = Chat::parse(local);
    let remote = Chat::parse(remote);

    let ids = |c: &Chat| c.entries.iter().map(|e| e.id).collect::<HashSet<_>>();
    let (base_ids, local_ids, remote_ids) = (ids(&base), ids(&local), ids(&remote));
    let deleted_locally: HashSet<_> = base_ids.difference(&local_ids).copied().collect();
    let deleted_remotely: HashSet<_> = base_ids.difference(&remote_ids).copied().collect();

    let mut entries: Vec<Entry> = local
        .entries
        .into_iter()
        .filter(|e| !deleted_remotely.contains(&e.id))
        .collect();
    entries.extend(
        remote
            .entries
            .into_iter()
            .filter(|e| !local_ids.contains(&e.id) && !deleted_locally.contains(&e.id)),
    );
    sort(&mut entries);
    Chat { entries }.serialize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn at(ts: i64, entry: Entry) -> Entry {
        Entry { ts, ..entry }
    }

    fn texts(bytes: &[u8]) -> Vec<String> {
        Chat::parse(bytes)
            .entries
            .iter()
            .map(|e| e.text().to_string())
            .collect()
    }

    #[test]
    fn round_trips_every_kind() {
        let mut chat = Chat::default();
        let mut user = at(1, Entry::user("a", "hi"));
        if let Body::User { spoken, mentions, .. } = &mut user.body {
            *spoken = true;
            mentions.push(Mention { path: "/x.md".into(), id: Some(Uuid::new_v4()) });
        }
        chat.push(user);
        chat.push(at(
            2,
            Entry::assistant("a", "hello", "p/m", Usage { input: 1, ..Default::default() }),
        ));
        chat.push(at(3, Entry::tool("a", "search", json!({"query": "q"}), "r", false)));
        chat.push(at(4, Entry::error("a", "boom")));
        chat.push(at(
            5,
            Entry::settings(
                "a",
                Settings {
                    model: Some("p/m".into()),
                    include: vec!["/i/".into()],
                    exclude: vec![],
                },
            ),
        ));

        assert_eq!(Chat::parse(&chat.serialize()), chat);
    }

    #[test]
    fn keeps_unknown_kinds_and_fields() {
        let line = "{\"id\":\"4d0b3f2a-0000-4000-8000-000000000001\",\"ts\":7,\"from\":\"a\",\"kind\":\"reaction\",\"emoji\":\"x\"}\n{\"id\":\"4d0b3f2a-0000-4000-8000-000000000002\",\"ts\":8,\"from\":\"a\",\"kind\":\"user\",\"text\":\"t\",\"mood\":3}\n";
        let chat = Chat::parse(line.as_bytes());
        assert_eq!(chat.entries.len(), 2);
        assert_eq!(chat.entries[0].body, Body::Other("reaction".into()));
        let out = String::from_utf8(chat.serialize()).unwrap();
        assert!(out.contains("\"emoji\":\"x\""), "{out}");
        assert!(out.contains("\"mood\":3"), "{out}");
        assert_eq!(Chat::parse(&chat.serialize()), chat);
    }

    #[test]
    fn push_keeps_commit_order_within_a_millisecond() {
        let mut chat = Chat::default();
        chat.push(at(5, Entry::user("a", "first")));
        chat.push(at(5, Entry::user("a", "second")));
        chat.push(at(1, Entry::user("a", "third")));
        assert_eq!(texts(&chat.serialize()), ["first", "second", "third"]);
        assert_eq!(Chat::parse(&chat.serialize()), chat);
    }

    #[test]
    fn orders_by_ts_then_id_and_drops_duplicates() {
        let late = at(5, Entry::user("a", "late"));
        let early = at(1, Entry::user("a", "early"));
        let bytes = [late.to_json_line(), early.to_json_line(), late.to_json_line()].join("\n");
        assert_eq!(texts(bytes.as_bytes()), ["early", "late"]);
    }

    #[test]
    fn latest_settings_win_per_user() {
        let mut chat = Chat::default();
        chat.push(at(
            1,
            Entry::settings("a", Settings { model: Some("one".into()), ..Default::default() }),
        ));
        chat.push(at(
            2,
            Entry::settings("b", Settings { model: Some("theirs".into()), ..Default::default() }),
        ));
        chat.push(at(
            3,
            Entry::settings("a", Settings { model: Some("two".into()), ..Default::default() }),
        ));
        assert_eq!(chat.settings_for("a").model.as_deref(), Some("two"));
        assert_eq!(chat.settings_for("b").model.as_deref(), Some("theirs"));
        assert_eq!(chat.settings_for("c"), Settings::default());
    }

    #[test]
    fn truncate_from_removes_the_entry_and_its_successors() {
        let mut chat = Chat::default();
        let keep = at(1, Entry::user("a", "keep"));
        let cut = at(2, Entry::user("a", "cut"));
        chat.push(keep);
        chat.push(cut.clone());
        chat.push(at(3, Entry::assistant("a", "gone", "m", Usage::default())));
        assert!(chat.truncate_from(cut.id));
        assert_eq!(texts(&chat.serialize()), ["keep"]);
        assert!(!chat.truncate_from(cut.id));
    }

    #[test]
    fn truncate_versus_append_converges() {
        let mut base = Chat::default();
        for (i, t) in ["a", "b", "c"].iter().enumerate() {
            base.push(at(i as i64, Entry::user("u", *t)));
        }
        let mut truncated = base.clone();
        truncated.truncate_from(base.entries[1].id);
        let mut appended = base.clone();
        appended.push(at(9, Entry::user("u", "d")));

        let (b, t, a) = (base.serialize(), truncated.serialize(), appended.serialize());
        assert_eq!(texts(&merge(&b, &t, &a)), ["a", "d"]);
        assert_eq!(merge(&b, &t, &a), merge(&b, &a, &t));
    }

    #[test]
    fn clear_versus_concurrent_send_converges() {
        let mut base = Chat::default();
        base.push(at(1, Entry::user("a", "one")));
        base.push(at(2, Entry::user("b", "two")));
        let mut extended = base.clone();
        extended.push(at(3, Entry::user("b", "three")));

        let (b, cleared, e) = (base.serialize(), Vec::new(), extended.serialize());
        assert_eq!(texts(&merge(&b, &e, &cleared)), ["three"]);
        assert_eq!(texts(&merge(&b, &cleared, &e)), ["three"]);
    }

    /// Random edits of a random base: the merge is symmetric, idempotent,
    /// never invents an entry, and never resurrects one a side deleted.
    #[test]
    fn merge_laws_hold_on_random_histories() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let mut base = Chat::default();
            for i in 0..rng.gen_range(0..6) {
                base.push(at(i, Entry::user("u", format!("b{i}"))));
            }
            let edit = |rng: &mut StdRng, tag: &str| {
                let mut c = base.clone();
                c.entries.shuffle(rng);
                c.entries.truncate(rng.gen_range(0..=c.entries.len()));
                sort(&mut c.entries);
                for i in 0..rng.gen_range(0..4) {
                    c.push(at(10 + i, Entry::user("u", format!("{tag}{i}"))));
                }
                c
            };
            let (x, y) = (edit(&mut rng, "x"), edit(&mut rng, "y"));
            let (b, xs, ys) = (base.serialize(), x.serialize(), y.serialize());

            assert_eq!(merge(&b, &xs, &ys), merge(&b, &ys, &xs));
            assert_eq!(merge(&b, &xs, &xs), x.serialize());

            let merged = Chat::parse(&merge(&b, &xs, &ys));
            let known: HashSet<_> = x.entries.iter().chain(&y.entries).map(|e| e.id).collect();
            for e in &merged.entries {
                assert!(known.contains(&e.id));
                let in_x = x.entries.iter().any(|m| m.id == e.id);
                let in_y = y.entries.iter().any(|m| m.id == e.id);
                let in_base = base.entries.iter().any(|m| m.id == e.id);
                assert!(!in_base || (in_x && in_y), "resurrected a deleted entry");
            }
        }
    }

    #[test]
    fn search_text_and_markdown_cover_what_was_said() {
        let mut chat = Chat::default();
        chat.push(at(1, Entry::user("a", "question")));
        chat.push(at(2, Entry::tool("a", "search", json!({}), "noise", true)));
        chat.push(at(3, Entry::assistant("a", "answer", "p/m", Usage::default())));
        chat.push(at(4, Entry::settings("a", Settings::default())));
        assert_eq!(chat.text(), "question\nanswer");
        let md = chat.to_markdown();
        assert!(md.contains("question") && md.contains("answer") && md.contains("`search`"));
    }
}
