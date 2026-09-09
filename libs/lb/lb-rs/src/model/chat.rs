//! Append-only chat log. Replay is the only constructor of transcript state.
//!
//! A `.chat` file is newline-delimited JSON events, merged across devices by
//! id-union (same shape as the old ts-union: concurrent appends combine;
//! a clear drops only events the clearer had seen). Credentials never live
//! here.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    User,
    Assistant,
    Thought,
    Tool,
    Plan,
    Ask,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl Status {
    pub fn in_flight(self) -> bool {
        matches!(self, Status::Open | Status::Pending | Status::Running)
    }

    pub fn terminal(self) -> bool {
        matches!(self, Status::Done | Status::Failed | Status::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechPhase {
    Start,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub operation: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text {
        text: String,
    },
    Image {
        mime: String,
        data: Vec<u8>,
    },
    Audio {
        mime: String,
        data: Vec<u8>,
    },
    Resource {
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime: Option<String>,
    },
    Link {
        uri: String,
        name: String,
    },
    Diff {
        changes: Vec<FileChange>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        patch: Option<String>,
    },
    Citations {
        uris: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
}

/// Token usage of the turn that produced an assistant item. `input` excludes
/// cached tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventBody {
    Open {
        item: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<Uuid>,
        kind: ItemKind,
    },
    Append {
        item: Uuid,
        block: Content,
    },
    Replace {
        item: Uuid,
        blocks: Vec<Content>,
    },
    SetStatus {
        item: Uuid,
        status: Status,
    },
    SetMeta {
        item: Uuid,
        meta: ItemMeta,
    },
    Speech {
        item: Uuid,
        phase: SpeechPhase,
    },
    Usage {
        item: Uuid,
        usage: Usage,
    },
}

/// One append-only log line. `from` is first-class so a human or group chat
/// is the same document as an agent transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub ts: i64,
    pub from: String,
    pub body: EventBody,
    /// Unknown top-level fields, preserved through merge.
    #[serde(flatten, default)]
    pub extra: Map<String, Value>,
}

impl Event {
    pub fn new(from: impl Into<String>, ts: i64, body: EventBody) -> Self {
        Self { id: Uuid::new_v4(), ts, from: from.into(), body, extra: Map::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub from: String,
    pub kind: ItemKind,
    pub status: Status,
    pub blocks: Vec<Content>,
    pub meta: ItemMeta,
    pub usage: Option<Usage>,
}

impl Item {
    pub fn text(&self) -> String {
        let mut s = String::new();
        for b in &self.blocks {
            if let Content::Text { text } = b {
                s.push_str(text);
            }
        }
        s
    }

    pub fn has_text(&self) -> bool {
        self.blocks
            .iter()
            .any(|b| matches!(b, Content::Text { text } if !text.is_empty()))
    }

    pub fn title(&self) -> String {
        self.meta
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| self.text())
    }
}

/// Folded snapshot. Display order is Open order, not log order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Transcript {
    pub items: Vec<Item>,
    pub user_floor: Option<Uuid>,
}

impl Transcript {
    pub fn fold(events: &[Event]) -> Self {
        let mut t = Self::default();
        for ev in events {
            t.apply(ev);
        }
        t
    }

    pub fn by_id(&self, id: Uuid) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn by_wire(&self, wire: &str) -> Option<Uuid> {
        self.items
            .iter()
            .find(|i| i.meta.wire_id.as_deref() == Some(wire))
            .map(|i| i.id)
    }

    fn by_id_mut(&mut self, id: Uuid) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    pub fn apply(&mut self, event: &Event) {
        match &event.body {
            EventBody::Open { item, parent, kind } => {
                if self.items.iter().any(|i| i.id == *item) {
                    return;
                }
                self.items.push(Item {
                    id: *item,
                    parent: *parent,
                    from: event.from.clone(),
                    kind: *kind,
                    status: Status::Open,
                    blocks: Vec::new(),
                    meta: ItemMeta::default(),
                    usage: None,
                });
            }
            EventBody::Append { item, block } => {
                if let Some(it) = self.by_id_mut(*item) {
                    it.blocks.push(block.clone());
                }
            }
            EventBody::Replace { item, blocks } => {
                if let Some(it) = self.by_id_mut(*item) {
                    it.blocks = blocks.clone();
                }
            }
            EventBody::SetStatus { item, status } => {
                if let Some(it) = self.by_id_mut(*item) {
                    it.status = *status;
                }
            }
            EventBody::SetMeta { item, meta } => {
                if let Some(it) = self.by_id_mut(*item) {
                    if meta.title.is_some() {
                        it.meta.title = meta.title.clone();
                    }
                    if meta.tool_kind.is_some() {
                        it.meta.tool_kind = meta.tool_kind.clone();
                    }
                    if meta.wire_id.is_some() {
                        it.meta.wire_id = meta.wire_id.clone();
                    }
                    if meta.args.is_some() {
                        it.meta.args = meta.args.clone();
                    }
                }
            }
            EventBody::Speech { item, phase } => match phase {
                SpeechPhase::Start => self.user_floor = Some(*item),
                SpeechPhase::Stop => {
                    if self.user_floor == Some(*item) {
                        self.user_floor = None;
                    }
                }
            },
            EventBody::Usage { item, usage } => {
                if let Some(it) = self.by_id_mut(*item) {
                    it.usage = Some(*usage);
                }
            }
        }
    }

    fn tools_of(&self, assistant: Uuid) -> impl Iterator<Item = &Item> {
        self.items
            .iter()
            .filter(move |i| i.kind == ItemKind::Tool && i.parent == Some(assistant))
    }

    fn is_provider_search(kind: Option<&str>) -> bool {
        matches!(
            kind,
            Some(
                "web_search"
                    | "web_search_with_snippets"
                    | "browse_page"
                    | "open_page"
                    | "open_page_with_find"
                    | "web_search_call"
                    | "search_images"
                    | "view_image"
                    | "x_search"
                    | "x_user_search"
                    | "x_keyword_search"
                    | "x_semantic_search"
                    | "x_thread_fetch"
                    | "x_search_call"
                    | "view_x_video"
                    | "code_interpreter"
                    | "code_interpreter_call"
                    | "code_execution"
                    | "code_execution_call"
            )
        )
    }

    fn unanswered_user(&self) -> bool {
        let last_user = self
            .items
            .iter()
            .rposition(|i| i.kind == ItemKind::User && i.status == Status::Done && i.has_text());
        let Some(idx) = last_user else {
            return false;
        };
        // Only a completed assistant counts. Failed / cancelled / in-flight
        // rows leave the user turn open for a retry.
        !self.items[idx + 1..]
            .iter()
            .any(|i| i.kind == ItemKind::Assistant && i.status == Status::Done)
    }

    pub fn tools_awaiting_follow_up(&self) -> bool {
        let Some(asst) = self
            .items
            .iter()
            .rev()
            .find(|i| i.kind == ItemKind::Assistant)
        else {
            return false;
        };
        if asst.status != Status::Done {
            return false;
        }
        let tools: Vec<&Item> = self
            .tools_of(asst.id)
            .filter(|t| !Self::is_provider_search(t.meta.tool_kind.as_deref()))
            .collect();
        if tools.is_empty() {
            return false;
        }
        if !tools.iter().all(|t| t.status.terminal()) {
            return false;
        }
        let asst_idx = self.items.iter().position(|i| i.id == asst.id).unwrap();
        !self.items[asst_idx + 1..]
            .iter()
            .any(|i| i.kind == ItemKind::Assistant)
    }

    pub fn on_turn_assistant(&self) -> Option<Uuid> {
        self.items
            .iter()
            .find(|i| i.kind == ItemKind::Assistant && i.status.in_flight())
            .map(|i| i.id)
    }

    /// Most recently opened assistant, in-flight or not. Server tools can
    /// land before assistant text (voice), so the parent is this when no
    /// turn is in flight yet.
    pub fn last_assistant(&self) -> Option<Uuid> {
        self.items
            .iter()
            .rev()
            .find(|i| i.kind == ItemKind::Assistant)
            .map(|i| i.id)
    }

    /// Whether a harness should create a model response. Derived, not stored.
    pub fn should_create(&self) -> bool {
        if self.user_floor.is_some() {
            return false;
        }
        if self.on_turn_assistant().is_some() {
            return false;
        }
        self.unanswered_user() || self.tools_awaiting_follow_up()
    }

    /// Model context: skip thought/plan/ask/error, skip in-flight/empty.
    pub fn context(&self) -> Vec<ChatMsg> {
        let mut out = Vec::new();
        for item in &self.items {
            if item.status.in_flight() {
                continue;
            }
            if item.status == Status::Cancelled && !item.has_text() {
                continue;
            }
            match item.kind {
                ItemKind::User if item.has_text() => {
                    out.push(ChatMsg::User { from: item.from.clone(), text: item.text() })
                }
                ItemKind::Assistant if item.status == Status::Done || item.has_text() => {
                    let tool_ids: Vec<String> = self
                        .tools_of(item.id)
                        .map(|t| t.meta.wire_id.clone().unwrap_or_else(|| t.id.to_string()))
                        .collect();
                    out.push(ChatMsg::Assistant { text: item.text(), tool_ids });
                }
                ItemKind::Tool if item.status == Status::Done || item.status == Status::Failed => {
                    out.push(ChatMsg::ToolResult {
                        id: item
                            .meta
                            .wire_id
                            .clone()
                            .unwrap_or_else(|| item.id.to_string()),
                        content: item.text(),
                        is_error: item.status == Status::Failed,
                    });
                }
                ItemKind::Thought | ItemKind::Plan | ItemKind::Ask | ItemKind::Error => {}
                _ => {}
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatMsg {
    User { from: String, text: String },
    Assistant { text: String, tool_ids: Vec<String> },
    ToolResult { id: String, content: String, is_error: bool },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Buffer {
    pub events: Vec<Event>,
}

impl Buffer {
    pub fn new(bytes: &[u8]) -> Self {
        let events: Vec<Event> = std::str::from_utf8(bytes)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() { None } else { serde_json::from_str(line).ok() }
            })
            .collect();
        Self { events }
    }

    pub fn transcript(&self) -> Transcript {
        Transcript::fold(&self.events)
    }

    pub fn merge(base: &[u8], local: &[u8], remote: &[u8]) -> Vec<u8> {
        let base = Self::new(base);
        let mut local = Self::new(local);
        let remote = Self::new(remote);

        let base_ids: std::collections::HashSet<Uuid> = base.events.iter().map(|e| e.id).collect();
        let local_ids: std::collections::HashSet<Uuid> =
            local.events.iter().map(|e| e.id).collect();
        let remote_ids: std::collections::HashSet<Uuid> =
            remote.events.iter().map(|e| e.id).collect();

        for ev in &remote.events {
            if !base_ids.contains(&ev.id) && !local_ids.contains(&ev.id) {
                local.events.push(ev.clone());
            }
        }

        local
            .events
            .retain(|ev| !base_ids.contains(&ev.id) || remote_ids.contains(&ev.id));
        local
            .events
            .sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.id.cmp(&b.id)));
        local.serialize()
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = self
            .events
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
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

    fn open(from: &str, ts: i64, item: Uuid, kind: ItemKind) -> Event {
        Event::new(from, ts, EventBody::Open { item, parent: None, kind })
    }

    fn text(from: &str, ts: i64, item: Uuid, s: &str) -> Event {
        Event::new(
            from,
            ts,
            EventBody::Replace { item, blocks: vec![Content::Text { text: s.into() }] },
        )
    }

    fn done(from: &str, ts: i64, item: Uuid) -> Event {
        Event::new(from, ts, EventBody::SetStatus { item, status: Status::Done })
    }

    fn failed(from: &str, ts: i64, item: Uuid) -> Event {
        Event::new(from, ts, EventBody::SetStatus { item, status: Status::Failed })
    }

    fn user_turn(from: &str, ts: i64, body: &str) -> Vec<Event> {
        let item = Uuid::from_u128(ts as u128);
        vec![
            open(from, ts, item, ItemKind::User),
            text(from, ts + 1, item, body),
            done(from, ts + 2, item),
        ]
    }

    fn ser(events: &[Event]) -> Vec<u8> {
        Buffer { events: events.to_vec() }.serialize()
    }

    #[test]
    fn merge_unions_concurrent_appends() {
        let hello = user_turn("a", 10, "hello");
        let one = {
            let mut v = hello.clone();
            v.extend(user_turn("a", 20, "one"));
            v
        };
        let two = {
            let mut v = hello.clone();
            v.extend(user_turn("b", 30, "two"));
            v
        };

        let merged = Buffer::new(&Buffer::merge(&ser(&hello), &ser(&one), &ser(&two)));
        let texts: Vec<_> = merged.transcript().items.iter().map(|i| i.text()).collect();
        assert_eq!(texts, ["hello", "one", "two"]);
    }

    #[test]
    fn clear_vs_concurrent_send_converges() {
        let mut base = user_turn("a", 10, "one");
        base.extend(user_turn("b", 20, "two"));
        let mut extended = base.clone();
        extended.extend(user_turn("b", 30, "three"));
        let cleared: Vec<Event> = Vec::new();

        let sender_view = Buffer::merge(&ser(&base), &ser(&extended), &ser(&cleared));
        let clearer_view = Buffer::merge(&ser(&base), &ser(&cleared), &ser(&extended));

        let texts = |bytes: &[u8]| {
            Buffer::new(bytes)
                .transcript()
                .items
                .iter()
                .map(|i| i.text())
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(&sender_view), ["three"]);
        assert_eq!(texts(&clearer_view), ["three"]);
    }

    #[test]
    fn merge_preserves_unknown_fields() {
        let item = Uuid::from_u128(1);
        let mut ev = open("a", 1, item, ItemKind::User);
        ev.extra
            .insert("reactions".into(), Value::String("abc".into()));
        let base = ser(&[ev.clone()]);
        let remote = {
            let mut ev2 = open("b", 2, Uuid::from_u128(2), ItemKind::User);
            ev2.extra
                .insert("reactions".into(), Value::String("xyz".into()));
            ser(&[ev, ev2])
        };
        let merged = String::from_utf8(Buffer::merge(&base, &base, &remote)).unwrap();
        assert!(merged.contains("\"reactions\":\"xyz\""), "unknown field stripped: {merged}");
    }

    #[test]
    fn fold_is_replay() {
        let item = Uuid::from_u128(7);
        let events = vec![
            open("travis", 1, item, ItemKind::User),
            text("travis", 2, item, "hi"),
            done("travis", 3, item),
        ];
        let t = Transcript::fold(&events);
        assert_eq!(t.items.len(), 1);
        assert_eq!(t.items[0].text(), "hi");
        assert_eq!(t.items[0].status, Status::Done);
        assert_eq!(t.items[0].from, "travis");
        assert!(t.should_create());
        assert_eq!(t.context(), vec![ChatMsg::User { from: "travis".into(), text: "hi".into() }]);
    }

    #[test]
    fn failed_assistant_still_needs_create() {
        let user = Uuid::from_u128(1);
        let asst = Uuid::from_u128(2);
        let err = Uuid::from_u128(3);
        let t = Transcript::fold(&[
            open("travis", 1, user, ItemKind::User),
            text("travis", 2, user, "hi"),
            done("travis", 3, user),
            open("travis", 4, asst, ItemKind::Assistant),
            failed("travis", 5, asst),
            open("travis", 6, err, ItemKind::Error),
            text("travis", 7, err, "400: no"),
            failed("travis", 8, err),
        ]);
        assert!(t.should_create());
    }

    #[test]
    fn done_assistant_does_not_need_create() {
        let user = Uuid::from_u128(1);
        let asst = Uuid::from_u128(2);
        let t = Transcript::fold(&[
            open("travis", 1, user, ItemKind::User),
            text("travis", 2, user, "hi"),
            done("travis", 3, user),
            open("travis", 4, asst, ItemKind::Assistant),
            text("travis", 5, asst, "hello"),
            done("travis", 6, asst),
        ]);
        assert!(!t.should_create());
    }

    fn open_child(from: &str, ts: i64, item: Uuid, parent: Uuid, kind: ItemKind) -> Event {
        Event::new(from, ts, EventBody::Open { item, parent: Some(parent), kind })
    }

    fn tool_kind(from: &str, ts: i64, item: Uuid, kind: &str) -> Event {
        Event::new(
            from,
            ts,
            EventBody::SetMeta {
                item,
                meta: ItemMeta {
                    title: Some(kind.into()),
                    tool_kind: Some(kind.into()),
                    wire_id: Some("c1".into()),
                    args: None,
                },
            },
        )
    }

    #[test]
    fn provider_search_does_not_need_follow_up() {
        let user = Uuid::from_u128(1);
        let asst = Uuid::from_u128(2);
        let tool = Uuid::from_u128(3);
        let t = Transcript::fold(&[
            open("travis", 1, user, ItemKind::User),
            text("travis", 2, user, "what's new"),
            done("travis", 3, user),
            open("travis", 4, asst, ItemKind::Assistant),
            text("travis", 5, asst, "here's the news"),
            done("travis", 6, asst),
            open_child("travis", 7, tool, asst, ItemKind::Tool),
            tool_kind("travis", 8, tool, "web_search"),
            text("travis", 9, tool, "web_search: xai"),
            done("travis", 10, tool),
        ]);
        assert!(!t.should_create());
        assert_eq!(t.last_assistant(), Some(asst));
    }

    #[test]
    fn client_tool_needs_follow_up() {
        let user = Uuid::from_u128(1);
        let asst = Uuid::from_u128(2);
        let tool = Uuid::from_u128(3);
        let t = Transcript::fold(&[
            open("travis", 1, user, ItemKind::User),
            text("travis", 2, user, "list notes"),
            done("travis", 3, user),
            open("travis", 4, asst, ItemKind::Assistant),
            done("travis", 5, asst),
            open_child("travis", 6, tool, asst, ItemKind::Tool),
            tool_kind("travis", 7, tool, "list"),
            text("travis", 8, tool, "notes/"),
            done("travis", 9, tool),
        ]);
        assert!(t.should_create());
    }

    #[test]
    fn set_meta_stamps_wire_on_existing_item() {
        let item = Uuid::from_u128(1);
        let t = Transcript::fold(&[
            open("travis", 1, item, ItemKind::User),
            Event::new(
                "travis",
                2,
                EventBody::SetMeta {
                    item,
                    meta: ItemMeta { wire_id: Some("s1".into()), ..ItemMeta::default() },
                },
            ),
        ]);
        assert_eq!(t.items.len(), 1);
        assert_eq!(t.items[0].id, item);
        assert_eq!(t.by_wire("s1"), Some(item));
    }

    #[test]
    fn duplicate_open_is_idempotent() {
        let item = Uuid::from_u128(1);
        let events =
            vec![open("a", 1, item, ItemKind::User), open("a", 2, item, ItemKind::Assistant)];
        let t = Transcript::fold(&events);
        assert_eq!(t.items.len(), 1);
        assert_eq!(t.items[0].kind, ItemKind::User);
    }
}
