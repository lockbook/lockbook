//! The `.chat` document: a first line of metadata (each user's settings),
//! then one JSON entry per line, merged across devices as a set union by
//! id. Only settled facts are written (a message that was sent, a reply that
//! finished, a tool call that returned), so nothing in flight is ever
//! reconciled.

use std::collections::{BTreeMap, BTreeSet, HashSet};

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

/// One user's settings for a chat. The folder the chat lives in is always
/// included and never listed.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Settings {
    /// `provider/model`; absent means the default provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    /// How hard the model thinks: one of the values its listing offers.
    /// Absent means the provider's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
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

    /// The text a reader sees; empty for unknown kinds.
    pub fn text(&self) -> &str {
        match &self.body {
            Body::User { text, .. } | Body::Assistant { text, .. } | Body::Error { text } => text,
            Body::Tool { result, .. } => result,
            Body::Other(_) => "",
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

/// The file's first line: what each user chose for this chat. A user writes
/// only their own entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub settings: BTreeMap<String, Settings>,
    /// Fields this client doesn't know, carried verbatim through merge and save.
    pub extra: Map<String, Value>,
}

impl Meta {
    fn absorb(&mut self, mut map: Map<String, Value>) {
        if let Some(settings) = take::<BTreeMap<String, Settings>>(&mut map, "settings") {
            self.settings.extend(settings);
        }
        self.extra.extend(map);
    }

    fn to_json_line(&self) -> Option<String> {
        let mut m = self.extra.clone();
        if !self.settings.is_empty() {
            m.insert("settings".into(), json!(self.settings));
        }
        if m.is_empty() {
            return None;
        }
        Some(serde_json::to_string(&Value::Object(m)).expect("maps serialize"))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chat {
    pub meta: Meta,
    /// Sorted by `(ts, id)`, ids unique.
    pub entries: Vec<Entry>,
}

impl Chat {
    /// A JSON object line with an id is an entry, kept whether or not its
    /// kind is known; one without an id is metadata; anything else is dropped.
    pub fn parse(bytes: &[u8]) -> Self {
        let text = String::from_utf8_lossy(bytes);
        let mut meta = Meta::default();
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for line in text.lines() {
            let Ok(Value::Object(map)) = serde_json::from_str::<Value>(line) else { continue };
            if !map.contains_key("id") {
                meta.absorb(map);
            } else if let Some(entry) = Entry::from_map(map) {
                if seen.insert(entry.id) {
                    entries.push(entry);
                }
            }
        }
        sort(&mut entries);
        Self { meta, entries }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = String::new();
        if let Some(meta) = self.meta.to_json_line() {
            out.push_str(&meta);
            out.push('\n');
        }
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

    /// Removes `id` and every later entry by `user`; other people's entries
    /// stay, so a rewrite in a shared chat only touches one's own history.
    /// Returns whether `id` was present.
    pub fn truncate_from(&mut self, id: Uuid, user: &str) -> bool {
        let Some(i) = self.entries.iter().position(|e| e.id == id) else { return false };
        let tail = self.entries.split_off(i);
        self.entries
            .extend(tail.into_iter().filter(|e| e.id != id && e.from != user));
        true
    }

    /// Removes `user`'s entries after their last message, keeping everyone
    /// else's. Returns whether there was a message to regenerate from.
    pub fn truncate_after_last_user(&mut self, user: &str) -> bool {
        let last = self
            .entries
            .iter()
            .rposition(|e| e.from == user && matches!(e.body, Body::User { .. }));
        let Some(i) = last else { return false };
        let tail = self.entries.split_off(i + 1);
        self.entries
            .extend(tail.into_iter().filter(|e| e.from != user));
        true
    }

    pub fn settings_for(&self, user: &str) -> Settings {
        self.meta.settings.get(user).cloned().unwrap_or_default()
    }

    pub fn set_settings(&mut self, user: &str, settings: Settings) {
        self.meta.settings.insert(user.to_string(), settings);
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
                Body::Other(_) => {}
            }
        }
        out
    }
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by_key(|e| (e.ts, e.id));
}

/// Three-way merge of two edits of `base`. Entries union by id; an entry one
/// side deleted from `base` stays deleted; identical ids take the local copy.
/// Each user's settings take whichever side changed them; if both sides did
/// (one user, two devices, between syncs), the local side wins.
pub fn merge(base: &[u8], local: &[u8], remote: &[u8]) -> Vec<u8> {
    let base = Chat::parse(base);
    let local = Chat::parse(local);
    let remote = Chat::parse(remote);

    let users: BTreeSet<&String> = [&base, &local, &remote]
        .into_iter()
        .flat_map(|c| c.meta.settings.keys())
        .collect();
    let mut settings = BTreeMap::new();
    for user in users {
        let (b, l, r) = (
            base.meta.settings.get(user),
            local.meta.settings.get(user),
            remote.meta.settings.get(user),
        );
        if let Some(chosen) = if l == b { r } else { l } {
            settings.insert(user.clone(), chosen.clone());
        }
    }
    let mut extra = remote.meta.extra.clone();
    extra.extend(local.meta.extra.clone());
    let meta = Meta { settings, extra };

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
    Chat { meta, entries }.serialize()
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
        chat.set_settings(
            "a",
            Settings {
                model: Some("p/m".into()),
                include: vec!["/i/".into()],
                exclude: vec![],
                effort: Some("high".into()),
            },
        );

        let bytes = chat.serialize();
        let first = String::from_utf8_lossy(&bytes)
            .lines()
            .next()
            .unwrap()
            .to_string();
        assert!(first.starts_with("{\"settings\":{\"a\":"), "{first}");
        assert_eq!(Chat::parse(&bytes), chat);
    }

    #[test]
    fn keeps_unknown_kinds_and_fields() {
        let line = "{\"settings\":{},\"theme\":\"dusk\"}\n{\"id\":\"4d0b3f2a-0000-4000-8000-000000000001\",\"ts\":7,\"from\":\"a\",\"kind\":\"reaction\",\"emoji\":\"x\"}\n{\"id\":\"4d0b3f2a-0000-4000-8000-000000000002\",\"ts\":8,\"from\":\"a\",\"kind\":\"user\",\"text\":\"t\",\"mood\":3}\n";
        let chat = Chat::parse(line.as_bytes());
        assert_eq!(chat.entries.len(), 2);
        assert_eq!(chat.entries[0].body, Body::Other("reaction".into()));
        let out = String::from_utf8(chat.serialize()).unwrap();
        assert!(out.starts_with("{\"theme\":\"dusk\"}\n"), "{out}");
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

    fn model(name: &str) -> Settings {
        Settings { model: Some(name.into()), ..Default::default() }
    }

    #[test]
    fn settings_are_per_user() {
        let mut chat = Chat::default();
        chat.set_settings("a", model("one"));
        chat.set_settings("b", model("theirs"));
        chat.set_settings("a", model("two"));
        assert_eq!(chat.settings_for("a").model.as_deref(), Some("two"));
        assert_eq!(chat.settings_for("b").model.as_deref(), Some("theirs"));
        assert_eq!(chat.settings_for("c"), Settings::default());
    }

    #[test]
    fn settings_merge_per_user_and_a_changed_side_wins() {
        let base = Chat::default();
        let mut local = base.clone();
        local.set_settings("a", model("a-local"));
        let mut remote = base.clone();
        remote.set_settings("b", model("b-remote"));
        let (b, l, r) = (base.serialize(), local.serialize(), remote.serialize());
        let merged = Chat::parse(&merge(&b, &l, &r));
        assert_eq!(merged.settings_for("a").model.as_deref(), Some("a-local"));
        assert_eq!(merged.settings_for("b").model.as_deref(), Some("b-remote"));
        assert_eq!(merge(&b, &l, &r), merge(&b, &r, &l));

        let mut remote = base.clone();
        remote.set_settings("a", model("a-remote"));
        let r = remote.serialize();
        assert_eq!(
            Chat::parse(&merge(&b, &b, &r))
                .settings_for("a")
                .model
                .as_deref(),
            Some("a-remote")
        );
        assert_eq!(
            Chat::parse(&merge(&b, &l, &r))
                .settings_for("a")
                .model
                .as_deref(),
            Some("a-local")
        );
    }

    #[test]
    fn truncate_from_removes_the_entry_and_its_successors() {
        let mut chat = Chat::default();
        let keep = at(1, Entry::user("a", "keep"));
        let cut = at(2, Entry::user("a", "cut"));
        chat.push(keep);
        chat.push(cut.clone());
        chat.push(at(3, Entry::assistant("a", "gone", "m", Usage::default())));
        assert!(chat.truncate_from(cut.id, "a"));
        assert_eq!(texts(&chat.serialize()), ["keep"]);
        assert!(!chat.truncate_from(cut.id, "a"));
    }

    #[test]
    fn rewrites_in_a_shared_chat_leave_others_lines_alone() {
        let mut chat = Chat::default();
        let mine = at(1, Entry::user("a", "mine"));
        chat.push(mine.clone());
        chat.push(at(2, Entry::assistant("a", "my reply", "m", Usage::default())));
        chat.push(at(3, Entry::user("b", "theirs")));
        chat.push(at(4, Entry::assistant("b", "their reply", "m", Usage::default())));
        chat.push(at(5, Entry::assistant("a", "my later reply", "m", Usage::default())));

        let mut regen = chat.clone();
        assert!(regen.truncate_after_last_user("a"));
        assert_eq!(texts(&regen.serialize()), ["mine", "theirs", "their reply"]);

        assert!(chat.truncate_from(mine.id, "a"));
        assert_eq!(texts(&chat.serialize()), ["theirs", "their reply"]);
        assert!(!chat.truncate_after_last_user("a"));
    }

    #[test]
    fn truncate_versus_append_converges() {
        let mut base = Chat::default();
        for (i, t) in ["a", "b", "c"].iter().enumerate() {
            base.push(at(i as i64, Entry::user("u", *t)));
        }
        let mut truncated = base.clone();
        truncated.truncate_from(base.entries[1].id, "u");
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
        assert_eq!(chat.text(), "question\nanswer");
        let md = chat.to_markdown();
        assert!(md.contains("question") && md.contains("answer") && md.contains("`search`"));
    }
}
