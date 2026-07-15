//! The agent's tools and the per-chat branch they edit.
//!
//! # The branch model
//!
//! An agent never touches a lockbook document directly. The first time it
//! observes a note's contents — a `read_note`, or an `edit_note`/`append_note`
//! that auto-opens — the note is *captured*: its bytes and version (hmac) are
//! snapshotted into a [`Buffer`] and the note surfaces as an agent-owned tab.
//! Every edit mutates the buffer, not the file. Each edit is individually
//! approved by the user (the card shows [`preview_edit`]'s diff); on
//! approval the harness runs the edit and immediately writes the buffer
//! back to the note ([`sync_note`], no longer a model tool).
//!
//! # Determinism / replay
//!
//! The buffer store is never persisted. It is a deterministic fold of the
//! recorded tool calls: captures load content, edits transform it, syncs mark
//! it clean. Replaying a chat's [`ToolRecord`](lb_rs::model::chat::ToolRecord)s
//! rebuilds every buffer, including unsaved ones. The load-bearing invariant:
//! **a record is a capture exactly when information flows disk → buffer**
//! (open, and a merge-case sync). Those records persist their content
//! un-truncated; everything else derives from the call itself.

use std::collections::BTreeMap;

use lb_rs::Lb;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::text::buffer::Buffer as TextBuffer;
use serde_json::{Value, json};

use super::backend::ToolSchema;

/// A note the model refuses to open beyond — a truncated capture would corrupt
/// replay, so oversized notes error instead of loading a partial snapshot.
const OPEN_CAP: usize = 256 * 1024;

/// The persisted result of a denied edit. Kept identical to what the model
/// sees, so a restored transcript's context matches the live one.
pub const DENIED_RESULT: &str = "The user denied this tool call.";

/// Whether `path` falls under any granted root: folder grants (trailing '/')
/// cover their subtree and the folder itself; note grants cover exactly that
/// note. `"/"` grants everything.
pub fn in_scope(path: &str, scope: &[String]) -> bool {
    let path = path.trim_end_matches('/');
    scope.iter().any(|g| match g.strip_suffix('/') {
        Some(folder) => {
            folder.is_empty() || path == folder || path.starts_with(&format!("{folder}/"))
        }
        None => path == g,
    })
}

/// The steering result of a call outside the chat's granted scope. Names the
/// recovery tool and states what's currently granted (the system prompt
/// deliberately doesn't — see `access_intro` — so this is the model's only
/// source for it short of scrolling its own history). Silent on whether the
/// path exists — existence outside scope is itself private.
pub fn out_of_scope(path: &str, scope: &[String]) -> String {
    let granted = if scope.is_empty() { "none yet".into() } else { scope.join(", ") };
    format!(
        "denied: {path} is outside the folders this chat can access. Currently granted: \
         {granted}. Call request_access to ask the user, then retry."
    )
}

/// A grant as stored: the canonical path, folders with their trailing '/'.
/// An unresolvable path is granted verbatim — harmless, and a folder created
/// later under a verbatim folder grant still matches.
pub(super) async fn normalize_grant(lb: &Lb, path: &str) -> String {
    match lb.get_by_path(path).await {
        Ok(f) => lb
            .get_path_by_id(f.id)
            .await
            .unwrap_or_else(|_| path.into()),
        Err(_) => path.into(),
    }
}

/// One note open on the branch: the captured base (content + version at
/// capture) and the current edited text. `dirty` ⇔ `text != base_text`.
#[derive(Clone)]
pub struct Buffer {
    /// Version of the last capture/sync. `None`: a new note not yet on disk.
    pub base_hmac: Option<DocumentHmac>,
    /// Content at the last capture/sync — the 3-way merge base.
    pub base_text: String,
    /// Current branch content.
    pub text: String,
}

impl Buffer {
    /// A fresh capture: branch starts equal to the captured base.
    fn captured(base_hmac: Option<DocumentHmac>, content: String) -> Self {
        Self { base_hmac, base_text: content.clone(), text: content }
    }

    pub fn dirty(&self) -> bool {
        self.text != self.base_text
    }
}

/// The agent's open notes, keyed by lockbook path. Owned by the driver for
/// the chat session's life; rebuilt on reopen by replaying tool records.
#[derive(Default)]
pub struct Buffers {
    open: BTreeMap<String, Buffer>,
}

impl Buffers {
    pub fn get(&self, path: &str) -> Option<&Buffer> {
        self.open.get(path)
    }

    pub fn is_open(&self, path: &str) -> bool {
        self.open.contains_key(path)
    }

    /// Load a capture into the store (an open, or a merge-case sync result).
    /// Used both by live dispatch and by replay.
    pub fn capture(&mut self, path: &str, base_hmac: Option<DocumentHmac>, content: String) {
        self.open
            .insert(path.to_string(), Buffer::captured(base_hmac, content));
    }

    /// Mark an open buffer saved at its current text (a clean sync).
    pub fn mark_clean(&mut self, path: &str) {
        if let Some(buf) = self.open.get_mut(path) {
            buf.base_text = buf.text.clone();
        }
    }

    /// Apply an exact-string edit to an open buffer. Returns the outcome line
    /// (with a size delta) or a steering error naming the match count.
    /// `replace_all` lifts the uniqueness requirement.
    pub fn edit(
        &mut self, path: &str, old: &str, new: &str, replace_all: bool,
    ) -> Result<String, String> {
        let Some(buf) = self.open.get_mut(path) else {
            return Err(format!("error: {path} isn't open. read_note it first."));
        };
        let count = buf.text.matches(old).count();
        match count {
            0 => Err(format!(
                "error: old_str not found in {path}. It must match exactly, \
                 including whitespace. read_note to see the current text."
            )),
            n if n > 1 && !replace_all => Err(format!(
                "error: old_str matches {n} places in {path}. Add surrounding \
                 context to make it unique, or set replace_all to change all {n}."
            )),
            _ => {
                let before = buf.text.lines().count();
                buf.text = if replace_all {
                    buf.text.replace(old, new)
                } else {
                    buf.text.replacen(old, new, 1)
                };
                Ok(edited_line(path, before, buf.text.lines().count()))
            }
        }
    }

    /// Append to an open buffer.
    pub fn append(&mut self, path: &str, content: &str) -> String {
        let buf = self.open.get_mut(path).expect("append on an open buffer");
        let before = buf.text.lines().count();
        if !buf.text.is_empty() && !buf.text.ends_with('\n') {
            buf.text.push('\n');
        }
        buf.text.push_str(content);
        edited_line(path, before, buf.text.lines().count())
    }

    /// Replace an open buffer's contents wholesale.
    pub fn write(&mut self, path: &str, content: String) -> String {
        let buf = self.open.get_mut(path).expect("write on an open buffer");
        let before = buf.text.lines().count();
        let after = content.lines().count();
        buf.text = content;
        edited_line(path, before, after)
    }
}

// The write-back follows approval immediately, so the model never sees an
// unsaved intermediate state.
fn edited_line(path: &str, before: usize, after: usize) -> String {
    format!("edited {path} — now {after} lines (was {before})")
}

/// Three-way merge of a sync conflict: reconcile our branch edits and the
/// concurrent on-disk edits over their common capture base, using the editor's
/// own merge so agent syncs behave like a human tab's save-conflict.
pub fn merge(base: &str, ours: &str, theirs: &str) -> String {
    TextBuffer::from(base).merge(ours.to_string(), theirs.to_string())
}

/// The result of one tool call, plus how the harness should persist it.
pub struct Outcome {
    /// What the model sees as the tool result (truncated on persist).
    pub result: String,
    /// `is_error` on the wire — a steering failure the model self-corrects.
    pub is_error: bool,
    /// Raw content to persist as this call's replay attachment, stored full
    /// (not truncated) and out of model context: the note a read/auto-open
    /// captured, or a merge-sync's merged text. On replay it loads the buffer
    /// (base+text) before this record's own mutation is folded in — so an
    /// auto-open edit stores the *pre-edit* content here, then the edit
    /// applies on top.
    pub capture: Option<String>,
}

impl Outcome {
    fn ok(result: String) -> Self {
        Self { result, is_error: false, capture: None }
    }

    fn err(result: String) -> Self {
        Self { result, is_error: true, capture: None }
    }
}

/// Replay one persisted tool record into the buffer store: load its capture
/// attachment (if any), then fold its own mutation. Deterministic — the same
/// records rebuild the same buffers. Rebuilt buffers carry no base hmac; their
/// first sync takes the merge path, which degenerates to a clean write when
/// disk is unchanged.
///
/// `is_error` (a steering failure or a denial) suppresses the *mutation* but
/// not the capture: a failed edit that auto-opened still leaves its note open
/// at the pre-edit base (matching live), while a denied sync must not mark the
/// buffer saved and a denied/errored call with no capture is a pure no-op.
pub fn fold_record(
    buffers: &mut Buffers, name: &str, args: &Value, capture: Option<String>, is_error: bool,
) {
    let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
    let had_capture = capture.is_some();
    if let Some(content) = capture {
        buffers.capture(path, None, content);
    }
    if is_error {
        return;
    }
    match name {
        "edit_note" => {
            let old = args
                .get("old_str")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let new = args
                .get("new_str")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let all = args
                .get("replace_all")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let _ = buffers.edit(path, old, new, all);
        }
        "append_note" => {
            if let Some(c) = args.get("content").and_then(Value::as_str) {
                if buffers.is_open(path) {
                    buffers.append(path, c);
                }
            }
        }
        "write_note" => {
            if let Some(c) = args.get("content").and_then(Value::as_str) {
                if buffers.is_open(path) {
                    buffers.write(path, c.to_string());
                }
            }
        }
        // A clean sync (no merged capture) just marks the buffer saved at its
        // current text; a merge sync's capture already set base+text = merged.
        "sync_note" if !had_capture => buffers.mark_clean(path),
        _ => {}
    }
}

/// How one recorded call renders in the transcript: a collapsed outcome
/// metric and an expanded body. Derived, never persisted — [`viz_record`]
/// produces it from the same fold replay uses, so what a row shows is exactly
/// the state the call left behind.
pub struct Viz {
    /// Right-aligned dim figure on the collapsed row ("42 lines", "+3 -1",
    /// "7 items", "saved", "error"). Empty renders nothing.
    pub metric: String,
    pub body: Body,
}

/// A tool row's expanded content.
#[derive(Clone)]
pub enum Body {
    /// Raw result text — unknown tools, steering errors, empty listings.
    Text(String),
    /// The affected note's new state — the value a read bound, rendered as
    /// markdown.
    Note { text: String },
    /// The change an edit made: stacked del/add segments with one block of
    /// surrounding context (the approval card's preview shape). The
    /// write-back receipt row that follows an approved edit carries no diff
    /// — the card just showed it.
    Rendered { segments: Vec<super::diff::Segment> },
    /// Listing rows; document paths open in the workspace on click.
    List { rows: Vec<String> },
}

/// Body lines beyond this are elided — an expanded row previews a value, the
/// note's own tab is the full view.
const BODY_LINES: usize = 200;

/// Fold one record (exactly like [`fold_record`]) and derive its row's
/// metric + expanded body from the before/after buffer states.
pub fn viz_record(
    buffers: &mut Buffers, name: &str, args: &Value, capture: Option<String>, is_error: bool,
    result: &str,
) -> Viz {
    let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
    let pre = buffers.get(path).cloned();
    let had_capture = capture.is_some();
    fold_record(buffers, name, args, capture, is_error);
    let post = buffers.get(path).cloned();

    let viz = |metric: &str, body: Body| Viz { metric: metric.into(), body };

    let text_body = || Body::Text(result.to_string());
    if result == DENIED_RESULT {
        return viz("denied", text_body());
    }
    if is_error {
        return viz("error", text_body());
    }
    match (name, post) {
        // Denied requests returned early above; what's left is a grant.
        ("request_access", _) => viz("granted", text_body()),
        ("list", _) => {
            if result.ends_with("is empty.") {
                return viz("empty", text_body());
            }
            let rows: Vec<String> = result.lines().map(str::to_string).collect();
            viz(&count(rows.len(), "item"), Body::List { rows })
        }
        ("read_note", Some(buf)) => {
            viz(&count(buf.text.lines().count(), "line"), Body::Note { text: elide(&buf.text) })
        }
        ("edit_note" | "append_note" | "write_note", Some(buf)) => {
            let old = pre.map(|b| b.text).unwrap_or_default();
            viz(
                &line_metric(&old, &buf.text),
                Body::Rendered { segments: super::diff::segments(&old, &buf.text, 1) },
            )
        }
        ("sync_note", Some(_)) => {
            // A receipt, not a diff — the approval card already showed the
            // change; repeating it here would render the same diff twice.
            let metric = if had_capture {
                "merged"
            } else if pre.is_some_and(|b| b.dirty()) {
                "saved"
            } else {
                "no changes"
            };
            viz(metric, text_body())
        }
        _ => viz("", text_body()),
    }
}

/// The change an `edit_note` call proposes, for the approval card: the
/// target's current text (open buffer, else disk) with the edit applied,
/// as del/add segments with one block of context. `Err` is the steering
/// message the call will hit if approved, shown on the card instead.
pub(super) async fn preview_edit(
    lb: &Lb, buffers: &Buffers, args: &Value,
) -> Result<Vec<super::diff::Segment>, String> {
    let path = arg(args, "path")?;
    let old = arg(args, "old_str")?;
    let new = arg(args, "new_str")?;
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let base = match buffers.get(path) {
        Some(b) => b.text.clone(),
        None => match capture_from_disk(lb, path).await {
            Ok((_, content)) if content.len() > OPEN_CAP => {
                return Err(format!(
                    "error: {path} is too large to open ({} KiB).",
                    content.len() / 1024
                ));
            }
            Ok((_, content)) => content,
            Err(NotADoc) => return Err(format!("error: {path} is a folder, not a note.")),
            Err(Missing) => return Err(format!("error: no note at {path}.")),
            Err(NotUtf8) => return Err(format!("error: {path} isn't text.")),
        },
    };
    // A scratch store so the preview can't touch the real branch.
    let mut scratch = Buffers::default();
    scratch.capture(path, None, base.clone());
    scratch.edit(path, old, new, replace_all)?;
    let text = scratch
        .get(path)
        .map(|b| b.text.clone())
        .unwrap_or_default();
    Ok(super::diff::segments(&base, &text, 1))
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

fn elide(text: &str) -> String {
    let mut lines = text.lines();
    let shown: Vec<&str> = lines.by_ref().take(BODY_LINES).collect();
    let rest = lines.count();
    if rest == 0 {
        text.to_string()
    } else {
        format!("{}\n… ({rest} more lines)", shown.join("\n"))
    }
}

/// "+added -removed" over a line diff, git-style — the collapsed row's
/// metric (lines are more informative than blocks there).
fn line_metric(old: &str, new: &str) -> String {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(old, new);
    let (mut add, mut del) = (0usize, 0usize);
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => add += 1,
            ChangeTag::Delete => del += 1,
            ChangeTag::Equal => {}
        }
    }
    format!("+{add} -{del}")
}

/// The minimal-slice tools. Order and content are stable across a
/// conversation — the schema block renders ahead of everything else in the
/// provider's prompt, so any change invalidates its cache.
pub fn schemas() -> Vec<ToolSchema> {
    let one_path = |prop: &str| {
        json!({
            "type": "object",
            "properties": { prop: { "type": "string" } },
            "required": [prop],
            "additionalProperties": false,
        })
    };
    vec![
        ToolSchema {
            name: "list",
            description: "List notes and folders. Folders end with '/'. Use \
                          this to explore the tree before reading.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder to list; defaults to the root." },
                    "recursive": { "type": "boolean" },
                },
                "required": [],
                "additionalProperties": false,
            }),
        },
        ToolSchema {
            name: "read_note",
            description: "Read a note's full contents. Paths are absolute \
                          and start with '/'.",
            parameters: one_path("path"),
        },
        ToolSchema {
            name: "request_access",
            description: "Ask the user to grant this chat access to a folder \
                          (path ending in '/') or a single note. Request the \
                          narrowest scope that covers the task — usually the \
                          chat's own folder. The user sees your reason.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "reason": { "type": "string", "description": "One short sentence: why you need it." },
                },
                "required": ["path"],
                "additionalProperties": false,
            }),
        },
        ToolSchema {
            name: "edit_note",
            description: "Replace an exact string in a note. old_str must \
                          match exactly once, or set replace_all to change \
                          every occurrence.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_str": { "type": "string" },
                    "new_str": { "type": "string" },
                    "replace_all": { "type": "boolean" },
                },
                "required": ["path", "old_str", "new_str"],
                "additionalProperties": false,
            }),
        },
    ]
}

/// A one-line human summary of a call, for the collapsed row and the
/// approval card — every parameter the model passed is reflected:
/// "read_note /todo.md", "list /notes/ (recursive)", "edit_note /a.md
/// (replace all)". Unknown shapes fall back to the raw args so nothing is
/// ever invisible.
pub fn detail_for(name: &str, args: &Value) -> String {
    let flag = |key: &str| args.get(key).and_then(Value::as_bool).unwrap_or(false);
    let path = args.get("path").and_then(Value::as_str);
    match (name, path) {
        ("list", p) => {
            let recursive = if flag("recursive") { " (recursive)" } else { "" };
            format!("list {}{recursive}", p.unwrap_or("/"))
        }
        ("edit_note", Some(p)) => {
            let all = if flag("replace_all") { " (replace all)" } else { "" };
            format!("edit_note {p}{all}")
        }
        (_, Some(p)) => format!("{name} {p}"),
        _ => {
            let mut s = format!("{name} {args}");
            if s.len() > 80 {
                let mut cut = 80;
                while !s.is_char_boundary(cut) {
                    cut -= 1;
                }
                s.truncate(cut);
                s.push('…');
            }
            s
        }
    }
}

/// Plain-language permission prose for the approval card, adapted to the
/// call's parameters and phrased as a yes/no question. Reads gate on *egress*
/// — the card only appears for a remote provider — so they name `provider`
/// and frame it as sending; `list` sends names, `read_note` sends contents.
/// Edits gate on *mutation* and head the diff below. Unknown shapes fall back
/// to the terse detail.
pub fn permission_prose(name: &str, args: &Value, provider: &str) -> String {
    let path = args.get("path").and_then(Value::as_str);
    let flag = |key: &str| args.get(key).and_then(Value::as_bool).unwrap_or(false);
    match name {
        "request_access" => match path.unwrap_or_default() {
            "/" => format!("Let {provider} read and edit your entire lockbook?"),
            p if p.ends_with('/') => format!("Let {provider} read and edit everything in {p}?"),
            p => format!("Let {provider} read and edit {p}?"),
        },
        "read_note" => {
            format!("Send the full contents of {} to {provider}?", path.unwrap_or_default())
        }
        "list" => {
            let folder = path.filter(|p| *p != "/");
            match (folder, flag("recursive")) {
                (None, false) => {
                    format!("Send {provider} the names of your top-level notes and folders?")
                }
                (Some(p), false) => {
                    format!("Send {provider} the names of the notes and folders in {p}?")
                }
                (None, true) => {
                    format!("Send {provider} the name of every note and folder in your lockbook?")
                }
                (Some(p), true) => {
                    format!("Send {provider} the name of every note and folder under {p}?")
                }
            }
        }
        "edit_note" => {
            let p = path.unwrap_or_default();
            if flag("replace_all") {
                format!("Apply this change everywhere it matches in {p}?")
            } else {
                format!("Apply this change to {p}?")
            }
        }
        _ => format!("Let the model run {}?", detail_for(name, args)),
    }
}

/// A `request_access` call's stated reason, trimmed — rendered on its own
/// line under [`permission_prose`]'s ask, set off in the model's own words
/// rather than folded into the yes/no sentence.
pub fn request_reason(args: &Value) -> Option<String> {
    let reason = args.get("reason").and_then(Value::as_str)?.trim();
    (!reason.is_empty()).then(|| reason.to_string())
}

/// A string argument, or a steering error naming the omission.
fn arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key).and_then(Value::as_str).ok_or_else(|| {
        // Malformed argument JSON arrives wrapped as {"raw": ...}.
        if let Some(raw) = args.get("raw") {
            format!("error: couldn't parse arguments ({raw}). Send valid JSON.")
        } else {
            format!("error: missing required argument '{key}'.")
        }
    })
}

/// Run one tool call against the branch and the lockbook. `lb` is the async
/// core (the driver runs its own runtime). Errors are folded into the result
/// so a failing call steers the model rather than aborting the turn.
pub async fn dispatch(lb: &Lb, buffers: &mut Buffers, name: &str, args: &Value) -> Outcome {
    match name {
        "list" => list(lb, args).await,
        "read_note" => read_note(lb, buffers, args).await,
        "edit_note" => edit_note(lb, buffers, args).await,
        other => Outcome::err(format!("error: no tool named '{other}'.")),
    }
}

async fn list(lb: &Lb, args: &Value) -> Outcome {
    let path = args.get("path").and_then(Value::as_str).unwrap_or("/");
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let folder = match lb.get_by_path(path).await {
        Ok(f) => f,
        Err(_) => return Outcome::err(did_you_mean(lb, path).await),
    };
    let mut rows: Vec<String> = Vec::new();
    let children = if recursive {
        lb.get_and_get_children_recursively(&folder.id).await
    } else {
        lb.get_children(&folder.id).await
    };
    let Ok(children) = children else {
        return Outcome::err(format!("error: couldn't list {path}."));
    };
    for f in children {
        if f.id == folder.id {
            continue;
        }
        let p = lb.get_path_by_id(f.id).await.unwrap_or(f.name.clone());
        rows.push(p);
    }
    if rows.is_empty() {
        return Outcome::ok(format!("{path} is empty."));
    }
    // Folders (trailing '/') first, then lexicographic — deterministic.
    rows.sort_by(|a, b| {
        let key = |s: &str| (!s.ends_with('/'), s.to_string());
        key(a).cmp(&key(b))
    });
    Outcome::ok(rows.join("\n"))
}

async fn read_note(lb: &Lb, buffers: &mut Buffers, args: &Value) -> Outcome {
    let path = match arg(args, "path") {
        Ok(p) => p,
        Err(e) => return Outcome::err(e),
    };
    // Already open: serve the branch view so a re-read reflects prior edits
    // (otherwise the model thinks its edit failed and repeats it). Not a new
    // capture.
    if let Some(buf) = buffers.get(path) {
        let n = buf.text.lines().count();
        return Outcome::ok(format!("[{path} — {n} lines, open]\n{}", buf.text));
    }
    match capture_from_disk(lb, path).await {
        Ok((hmac, content)) => {
            if content.len() > OPEN_CAP {
                return Outcome::err(format!(
                    "error: {path} is too large to open ({} KiB). Not supported yet.",
                    content.len() / 1024
                ));
            }
            buffers.capture(path, hmac, content.clone());
            let n = content.lines().count();
            let mut out = Outcome::ok(format!("[{path} — {n} lines]\n{content}"));
            // The raw content is the replay attachment (the headered result the
            // model saw is truncated on persist; the buffer needs the whole
            // note).
            out.capture = Some(content);
            out
        }
        Err(NotADoc) => Outcome::err(format!("error: {path} is a folder, not a note.")),
        Err(Missing) => Outcome::err(did_you_mean(lb, path).await),
        Err(NotUtf8) => Outcome::err(format!("error: {path} isn't text.")),
    }
}

async fn edit_note(lb: &Lb, buffers: &mut Buffers, args: &Value) -> Outcome {
    let (path, old, new) = match (arg(args, "path"), arg(args, "old_str"), arg(args, "new_str")) {
        (Ok(p), Ok(o), Ok(n)) => (p, o, n),
        (Err(e), ..) | (_, Err(e), _) | (.., Err(e)) => return Outcome::err(e),
    };
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // Auto-open an untouched note. The pre-edit snapshot rides this record as
    // its capture attachment (context-excluded — the model is editing blind, a
    // zero-egress transform); on replay it loads the base before the edit
    // folds on top.
    let capture = if buffers.is_open(path) {
        None
    } else {
        match capture_from_disk(lb, path).await {
            Ok((hmac, content)) => {
                if content.len() > OPEN_CAP {
                    return Outcome::err(format!(
                        "error: {path} is too large to open ({} KiB).",
                        content.len() / 1024
                    ));
                }
                buffers.capture(path, hmac, content.clone());
                Some(content)
            }
            Err(NotADoc) => return Outcome::err(format!("error: {path} is a folder.")),
            Err(Missing) => return Outcome::err(did_you_mean(lb, path).await),
            Err(NotUtf8) => return Outcome::err(format!("error: {path} isn't text.")),
        }
    };

    match buffers.edit(path, old, new, replace_all) {
        Ok(result) => Outcome { result, is_error: false, capture },
        // A failed edit still leaves the auto-open capture in place — the note
        // is open now, so persist the capture and let the model retry.
        Err(result) => Outcome { result, is_error: true, capture },
    }
}

/// Write a buffer back to the real note — no longer a model tool; the
/// driver runs it right after each approved edit.
pub(super) async fn sync_note(lb: &Lb, buffers: &mut Buffers, args: &Value) -> Outcome {
    let path = match arg(args, "path") {
        Ok(p) => p,
        Err(e) => return Outcome::err(e),
    };
    let Some(buf) = buffers.get(path).cloned() else {
        return Outcome::err(format!("error: {path} isn't open, nothing to sync."));
    };
    if !buf.dirty() {
        return Outcome::ok(format!("{path} has no unsaved changes."));
    }
    match write_back(lb, path, &buf).await {
        Ok(Synced { new_hmac, merged }) => {
            let content = merged.clone().unwrap_or(buf.text);
            buffers.capture(path, Some(new_hmac), content.clone());
            match merged {
                // The merge pulled in concurrent on-disk edits — disk → buffer
                // inflow — so the merged text rides the record as its capture
                // attachment (replay sets base+text = merged, clean).
                Some(_) => Outcome {
                    result: format!("saved {path} (merged with changes made since you opened it)."),
                    is_error: false,
                    capture: Some(content),
                },
                None => Outcome::ok(format!("saved {path}.")),
            }
        }
        Err(e) => Outcome::err(format!("error: couldn't save {path}: {e}.")),
    }
}

struct Synced {
    new_hmac: DocumentHmac,
    /// The merged text when a concurrent edit forced a 3-way merge; `None` on
    /// a clean write.
    merged: Option<String>,
}

/// Write a dirty buffer back with compare-and-swap. On a concurrent on-disk
/// change, 3-way merge and retry. A brand-new note (no base hmac) is created
/// first.
async fn write_back(lb: &Lb, path: &str, buf: &Buffer) -> Result<Synced, String> {
    let id = match lb.get_by_path(path).await {
        Ok(f) => f.id,
        // Never synced: create it, then write with no prior version.
        Err(_) if buf.base_hmac.is_none() => {
            lb.create_at_path(path).await.map_err(|e| e.to_string())?.id
        }
        Err(e) => return Err(e.to_string()),
    };

    // CAS against the disk version, merging in anything that landed since our
    // capture. Loop in case disk moves again between read and write.
    loop {
        let (disk_hmac, disk_bytes) = lb
            .read_document_with_hmac(id, false)
            .await
            .map_err(|e| e.to_string())?;
        let clean = disk_hmac == buf.base_hmac;
        let to_write = if clean {
            buf.text.clone()
        } else {
            let disk_text = String::from_utf8_lossy(&disk_bytes);
            merge(&buf.base_text, &buf.text, &disk_text)
        };
        match lb
            .safe_write(id, disk_hmac, to_write.clone().into_bytes(), None)
            .await
        {
            Ok(new_hmac) => {
                return Ok(Synced { new_hmac, merged: (!clean).then_some(to_write) });
            }
            // Disk changed again between read and write — re-read and retry.
            Err(e) if is_reread(&e) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn is_reread(e: &lb_rs::model::errors::LbErr) -> bool {
    matches!(e.kind, lb_rs::model::errors::LbErrKind::ReReadRequired)
}

enum CaptureErr {
    Missing,
    NotADoc,
    NotUtf8,
}
use CaptureErr::*;

/// Read a note's bytes and version for capture. A never-written note reads as
/// empty (not an error) — the model can edit a blank buffer and create it on
/// sync.
async fn capture_from_disk(
    lb: &Lb, path: &str,
) -> Result<(Option<DocumentHmac>, String), CaptureErr> {
    let file = lb.get_by_path(path).await.map_err(|_| Missing)?;
    if !file.is_document() {
        return Err(NotADoc);
    }
    let (hmac, bytes) = lb
        .read_document_with_hmac(file.id, true)
        .await
        .map_err(|_| Missing)?;
    let content = String::from_utf8(bytes).map_err(|_| NotUtf8)?;
    Ok((hmac, content))
}

/// A path miss with the closest actual paths, so the model can self-correct.
async fn did_you_mean(lb: &Lb, path: &str) -> String {
    let mut suggestions = Vec::new();
    if let Ok(paths) = lb.list_paths(None).await {
        let target = path.to_lowercase();
        let mut scored: Vec<(usize, String)> = paths
            .into_iter()
            .map(|p| {
                let overlap = p
                    .to_lowercase()
                    .chars()
                    .filter(|c| target.contains(*c))
                    .count();
                (overlap, p)
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        suggestions = scored.into_iter().take(3).map(|(_, p)| p).collect();
    }
    if suggestions.is_empty() {
        format!("error: no note at {path}.")
    } else {
        format!("error: no note at {path}. Closest: {}.", suggestions.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(buffers: &mut Buffers, path: &str, content: &str) {
        buffers.capture(path, Some([0u8; 32]), content.to_string());
    }

    /// Edit requires a unique match unless replace_all; a miss and an
    /// ambiguous match both steer instead of mutating.
    #[test]
    fn edit_uniqueness_and_steering() {
        let mut b = Buffers::default();
        open(&mut b, "/n.md", "a\nb\na\n");

        let miss = b.edit("/n.md", "z", "y", false).unwrap_err();
        assert!(miss.contains("not found"), "{miss}");

        let ambiguous = b.edit("/n.md", "a", "x", false).unwrap_err();
        assert!(ambiguous.contains("matches 2"), "{ambiguous}");
        // A steering failure left the buffer untouched.
        assert_eq!(b.get("/n.md").unwrap().text, "a\nb\na\n");

        b.edit("/n.md", "a", "x", true).unwrap();
        assert_eq!(b.get("/n.md").unwrap().text, "x\nb\nx\n");

        let mut b2 = Buffers::default();
        open(&mut b2, "/n.md", "a\nb\na\n");
        b2.edit("/n.md", "b", "B", false).unwrap();
        assert_eq!(b2.get("/n.md").unwrap().text, "a\nB\na\n");
    }

    /// Edit on an unopened path steers (dispatch handles auto-open; the pure
    /// buffer op does not).
    #[test]
    fn edit_unopened_steers() {
        let mut b = Buffers::default();
        let e = b.edit("/nope.md", "a", "b", false).unwrap_err();
        assert!(e.contains("isn't open"), "{e}");
    }

    /// dirty tracks divergence from the captured base; append newline-joins.
    #[test]
    fn append_and_dirty() {
        let mut b = Buffers::default();
        open(&mut b, "/n.md", "line one");
        assert!(!b.get("/n.md").unwrap().dirty());
        b.append("/n.md", "line two");
        assert_eq!(b.get("/n.md").unwrap().text, "line one\nline two");
        assert!(b.get("/n.md").unwrap().dirty());
    }

    /// Non-overlapping branch and disk edits both survive the 3-way merge — a
    /// self-referential property (each side's change is present in the result).
    #[test]
    fn merge_keeps_both_sides() {
        let base = "line one\nline two\nline three\n";
        let ours = "line one EDITED\nline two\nline three\n";
        let theirs = "line one\nline two\nline three ALSO\n";
        let merged = merge(base, ours, theirs);
        assert!(merged.contains("line one EDITED"), "ours lost: {merged}");
        assert!(merged.contains("line three ALSO"), "theirs lost: {merged}");
    }

    /// A merge with no concurrent change (theirs == base) is exactly our text
    /// — the clean path and the merge path agree at the boundary.
    #[test]
    fn merge_no_conflict_is_ours() {
        let base = "a\nb\nc\n";
        let ours = "a\nB\nc\n";
        assert_eq!(merge(base, ours, base), ours);
    }

    /// Replaying a capture then an edit reconstructs the same buffer the live
    /// run produced — the determinism the persistence layer relies on.
    #[test]
    fn replay_reconstructs_buffer() {
        // Live: capture "v1", edit.
        let mut live = Buffers::default();
        live.capture("/n.md", Some([7u8; 32]), "hello world".into());
        live.edit("/n.md", "world", "there", false).unwrap();

        // Replay: same capture record, same edit record.
        let mut replay = Buffers::default();
        replay.capture("/n.md", Some([7u8; 32]), "hello world".into());
        replay.edit("/n.md", "world", "there", false).unwrap();

        assert_eq!(live.get("/n.md").unwrap().text, replay.get("/n.md").unwrap().text);
        assert_eq!(replay.get("/n.md").unwrap().text, "hello there");
    }

    /// `fold_record` rebuilds buffers from persisted records: a read captures,
    /// an unsynced edit leaves the buffer dirty (the reopen money moment), and
    /// a clean sync marks it saved.
    #[test]
    fn fold_records_rebuilds_dirty_and_clean() {
        // read /a.md (capture attachment = content), then edit it, no sync.
        let mut b = Buffers::default();
        fold_record(
            &mut b,
            "read_note",
            &json!({ "path": "/a.md" }),
            Some("hello world".into()),
            false,
        );
        fold_record(
            &mut b,
            "edit_note",
            &json!({ "path": "/a.md", "old_str": "world", "new_str": "there" }),
            None,
            false,
        );
        let a = b.get("/a.md").unwrap();
        assert_eq!(a.text, "hello there");
        assert!(a.dirty(), "unsynced edit must reopen dirty");

        // A blind edit auto-opens: the pre-edit content rides the record's
        // capture; then a clean sync marks it saved.
        fold_record(
            &mut b,
            "edit_note",
            &json!({ "path": "/b.md", "old_str": "Q3", "new_str": "Q4" }),
            Some("plan Q3".into()),
            false,
        );
        assert_eq!(b.get("/b.md").unwrap().text, "plan Q4");
        assert!(b.get("/b.md").unwrap().dirty());
        fold_record(&mut b, "sync_note", &json!({ "path": "/b.md" }), None, false);
        assert!(!b.get("/b.md").unwrap().dirty(), "clean sync marks saved");

        // A DENIED sync must not mark the buffer clean on replay.
        fold_record(
            &mut b,
            "edit_note",
            &json!({ "path": "/a.md", "old_str": "there", "new_str": "gone" }),
            None,
            false,
        );
        assert!(b.get("/a.md").unwrap().dirty());
        fold_record(&mut b, "sync_note", &json!({ "path": "/a.md" }), None, true);
        assert!(b.get("/a.md").unwrap().dirty(), "denied sync must stay dirty");
    }

    /// A merge-sync record carries the merged text as its capture, so replay
    /// restores the post-merge buffer (clean).
    #[test]
    fn fold_merge_sync_restores_merged() {
        let mut b = Buffers::default();
        fold_record(&mut b, "read_note", &json!({ "path": "/m.md" }), Some("base".into()), false);
        fold_record(
            &mut b,
            "sync_note",
            &json!({ "path": "/m.md" }),
            Some("merged text".into()),
            false,
        );
        let m = b.get("/m.md").unwrap();
        assert_eq!(m.text, "merged text");
        assert!(!m.dirty());
    }

    /// viz_record folds exactly like fold_record and derives each row's
    /// metric + body from the before/after states: a read binds the note as
    /// its body, an edit shows a +/- diff, a sync reports how it saved.
    #[test]
    fn viz_derives_metrics_and_bodies() {
        let mut b = Buffers::default();

        let read = viz_record(
            &mut b,
            "read_note",
            &json!({ "path": "/a.md" }),
            Some("one\ntwo\n".into()),
            false,
            "[/a.md — 2 lines]\none\ntwo",
        );
        assert_eq!(read.metric, "2 lines");
        match read.body {
            Body::Note { text } => assert_eq!(text, "one\ntwo\n"),
            _ => panic!("read body should be the bound note"),
        }

        let edit = viz_record(
            &mut b,
            "edit_note",
            &json!({ "path": "/a.md", "old_str": "two", "new_str": "TWO\nthree" }),
            None,
            false,
            "edited /a.md",
        );
        assert_eq!(edit.metric, "+2 -1");
        match edit.body {
            Body::Rendered { segments } => {
                use super::super::diff::SegKind::*;
                let flat: Vec<_> = segments.iter().map(|s| (s.text.as_str(), s.kind)).collect();
                assert_eq!(flat, [("one\ntwo", Del), ("one\nTWO\nthree", Add)]);
            }
            _ => panic!("edit body should be a rendered diff"),
        }

        // The write-back receipt row is a receipt, not a diff (the approval
        // card already showed the change). Folding it marks the buffer clean.
        let sync = viz_record(
            &mut b,
            "sync_note",
            &json!({ "path": "/a.md" }),
            None,
            false,
            "saved /a.md.",
        );
        assert_eq!(sync.metric, "saved");
        assert!(matches!(sync.body, Body::Text(_)));
        assert!(!b.get("/a.md").unwrap().dirty());

        let denied =
            viz_record(&mut b, "sync_note", &json!({ "path": "/a.md" }), None, true, DENIED_RESULT);
        assert_eq!(denied.metric, "denied");

        let list = viz_record(&mut b, "list", &json!({}), None, false, "/docs/\n/a.md\n/b.md");
        assert_eq!(list.metric, "3 items");
        assert!(matches!(list.body, Body::List { ref rows } if rows.len() == 3));
    }

    /// Permission prose adapts to each call's parameters: reads name the
    /// provider and frame egress; list distinguishes root/folder × shallow/
    /// recursive; edits caption the diff.
    #[test]
    fn permission_prose_adapts_to_params() {
        let p = |name: &str, args: Value| permission_prose(name, &args, "Cerebras");

        assert_eq!(
            p("read_note", json!({ "path": "/todo.md" })),
            "Send the full contents of /todo.md to Cerebras?"
        );

        assert_eq!(
            p("list", json!({})),
            "Send Cerebras the names of your top-level notes and folders?"
        );
        assert_eq!(
            p("list", json!({ "path": "/" })),
            "Send Cerebras the names of your top-level notes and folders?"
        );
        assert_eq!(
            p("list", json!({ "path": "/work/" })),
            "Send Cerebras the names of the notes and folders in /work/?"
        );
        assert_eq!(
            p("list", json!({ "recursive": true })),
            "Send Cerebras the name of every note and folder in your lockbook?"
        );
        assert_eq!(
            p("list", json!({ "path": "/work/", "recursive": true })),
            "Send Cerebras the name of every note and folder under /work/?"
        );

        assert_eq!(p("edit_note", json!({ "path": "/a.md" })), "Apply this change to /a.md?");
        assert_eq!(
            p("edit_note", json!({ "path": "/a.md", "replace_all": true })),
            "Apply this change everywhere it matches in /a.md?"
        );

        assert_eq!(p("mystery", json!({})), "Let the model run mystery {}?");
    }

    /// The summary line reflects every parameter the model passed —
    /// flags parenthesized, unknown shapes falling back to raw args.
    #[test]
    fn detail_reflects_params() {
        assert_eq!(detail_for("list", &json!({})), "list /");
        assert_eq!(
            detail_for("list", &json!({ "path": "/notes/", "recursive": true })),
            "list /notes/ (recursive)"
        );
        assert_eq!(detail_for("read_note", &json!({ "path": "/a.md" })), "read_note /a.md");
        assert_eq!(
            detail_for(
                "edit_note",
                &json!({ "path": "/a.md", "old_str": "x", "new_str": "y", "replace_all": true })
            ),
            "edit_note /a.md (replace all)"
        );
        // Unknown shape: raw args shown, truncated.
        let detail = detail_for("mystery", &json!({ "q": "z".repeat(200) }));
        assert!(detail.starts_with("mystery {\"q\""));
        assert!(detail.ends_with('…') && detail.len() < 100);
    }

    /// The slice tools are advertised, with flat strict-compatible
    /// schemas (object, additionalProperties:false).
    #[test]
    fn schemas_are_flat_and_named() {
        let names: Vec<_> = schemas().iter().map(|s| s.name).collect();
        assert_eq!(names, ["list", "read_note", "request_access", "edit_note"]);
        for s in schemas() {
            assert_eq!(s.parameters["type"], "object");
            assert_eq!(s.parameters["additionalProperties"], false);
        }
    }

    /// Folder grants cover their subtree and the folder itself, on path
    /// boundaries; note grants cover exactly that note; "/" covers all.
    #[test]
    fn in_scope_matches_on_path_boundaries() {
        let scope = |s: &[&str]| s.iter().map(|g| g.to_string()).collect::<Vec<_>>();

        let folder = scope(&["/notes/"]);
        assert!(in_scope("/notes/a.md", &folder));
        assert!(in_scope("/notes/deep/b.md", &folder));
        assert!(in_scope("/notes/", &folder), "listing the granted folder itself");
        assert!(in_scope("/notes", &folder), "trailing-slash-less spelling");
        assert!(!in_scope("/notes2/a.md", &folder), "sibling prefix must not match");
        assert!(!in_scope("/a.md", &folder));

        let note = scope(&["/one.md"]);
        assert!(in_scope("/one.md", &note));
        assert!(!in_scope("/one.md.bak", &note));
        assert!(!in_scope("/", &note));

        let root = scope(&["/"]);
        assert!(in_scope("/", &root));
        assert!(in_scope("/anything/x.md", &root));

        assert!(!in_scope("/anything", &[]), "empty scope grants nothing");
    }
}
