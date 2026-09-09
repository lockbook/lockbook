//! Client tools are the chat action surface: named functions with JSON
//! params, shared by typed chat and voice. Slash commands or keymaps can
//! call the same `run` / `settings` later — don't add a second dispatcher.

use lb_rs::blocking::Lb;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::media_text;
use lb_rs::model::usage::bytes_to_human;
use lb_rs::search::{SearchFilter, SearchResult};
use lb_rs::service::activity::RankingWeights;
use serde_json::{Value, json};

use tracing::{info, warn};

use crate::file_cache::path_segments;

pub const DEFAULT_VOICE: &str = "eve";
pub const VOICES: &[&str] = &[
    "ara", "eve", "leo", "rex", "sal", "altair", "atlas", "aurora", "carina", "castor", "celeste",
    "cosmo", "helios", "helix", "iris", "kepler", "liora", "lumen", "luna", "lux", "naksh",
    "orion", "perseus", "rigel", "sirius", "ursa", "zagan", "zenith",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsState {
    pub voice: String,
    pub input: String,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabSnap {
    pub file_id: lb_rs::Uuid,
    pub path: String,
    pub active: bool,
    pub live: bool,
    pub can_back: bool,
    pub can_forward: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabOp {
    Open(lb_rs::Uuid),
    Close(lb_rs::Uuid),
    Move { id: lb_rs::Uuid, to: usize },
    Back,
    Forward,
}

#[derive(Clone, Debug)]
pub struct ClientToolOut {
    pub text: String,
    pub image: Option<(String, Vec<u8>)>,
}

impl ClientToolOut {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), image: None }
    }
}

const READ_CAP: usize = 256 * 1024;
const SEARCH_CAP: usize = 20;
const LIST_CAP: usize = 100;
/// Search-UI snippets use 30 chars of context (file-row caption). Tool results
/// need a readable span; 80/side is ~200–300 with the match, then SNIPPET_CAP.
const SNIPPET_CONTEXT: usize = 80;
const SNIPPET_CAP: usize = 280;

/// Realtime `response.function_call_arguments.done`. Same event typed HTTP uses.
pub fn parse_call(ev: &Value) -> Option<(String, String, Value)> {
    if ev.get("type").and_then(|v| v.as_str()) != Some("response.function_call_arguments.done") {
        return None;
    }
    let id = ev.get("call_id")?.as_str()?.to_string();
    let name = ev.get("name")?.as_str()?.to_string();
    let args = match ev.get("arguments") {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_else(|_| json!({ "raw": s })),
        Some(other) => other.clone(),
        None => json!({}),
    };
    Some((id, name, args))
}

pub fn is_client(name: &str) -> bool {
    matches!(
        name,
        "list"
            | "read"
            | "info"
            | "this"
            | "edit"
            | "search"
            | "create"
            | "rename"
            | "move"
            | "delete"
            | "recent"
            | "pin"
            | "duplicate"
            | "share"
            | "contacts"
            | "account"
            | "now"
            | "settings"
            | "tabs"
            | "hangup"
            | "imagine"
            | "look"
            | "download"
            | "transcribe"
            | "record"
            | "caption"
            | "transcript"
    )
}

pub fn is_server(name: &str) -> bool {
    matches!(
        name,
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
}

fn web_search_tool() -> Value {
    json!({ "type": "web_search", "enable_image_search": true })
}

fn x_search_tool() -> Value {
    json!({ "type": "x_search" })
}

/// Typed chat: server search/code plus client functions.
pub fn typed_tools() -> Value {
    let mut out = vec![web_search_tool(), x_search_tool(), json!({ "type": "code_interpreter" })];
    if let Some(fns) = function_tools().as_array() {
        out.extend(fns.iter().cloned());
    }
    Value::Array(out)
}

/// Live call: no code interpreter (realtime 400s). Same client functions.
pub fn call_tools() -> Value {
    let mut out = vec![web_search_tool(), x_search_tool()];
    if let Some(fns) = function_tools().as_array() {
        out.extend(fns.iter().cloned());
    }
    Value::Array(out)
}

/// Built-in xAI tools plus Lockbook client functions. Provider tools
/// (`web_search`, `x_search`, `code_interpreter`) run on the server;
/// we never send a tool result.
pub fn all_tools() -> Value {
    typed_tools()
}

/// Responses / Realtime function shape (`name` at the top, not nested).
fn fn_tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({
        "type": "function",
        "name": name,
        "description": description,
        "parameters": parameters,
    })
}

pub fn function_tools() -> Value {
    Value::Array(vec![
        fn_tool(
            "list",
            "List a Lockbook folder (non-recursive). Directories are suffixed with '/'. Omit path for the root.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder path, e.g. /notes" }
                }
            }),
        ),
        fn_tool(
            "read",
            "Read a UTF-8 Lockbook note (capped at 256KiB).",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path, e.g. /notes/todo.md" }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "info",
            "Metadata for a note or folder: type, size, last modified, pin, owner, shares. Folders include a child count, not recursive size. `.` is this chat.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File or folder path. `.` is this chat. Omit for the root." }
                }
            }),
        ),
        fn_tool(
            "this",
            "Where this conversation lives (path, pin, size, modified). Details change; call when needed instead of guessing.",
            json!({ "type": "object", "properties": {} }),
        ),
        fn_tool(
            "edit",
            "Replace `old` with `new` in a note. `old` must occur exactly once. Empty `old` creates or overwrites the file with `new`.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old": { "type": "string" },
                    "new": { "type": "string" }
                },
                "required": ["path", "old", "new"]
            }),
        ),
        fn_tool(
            "search",
            "Find Lockbook files by filename (fuzzy) or by contents (notes, chats, image captions, audio transcripts). Prefer this over listing folders recursively.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "in": {
                        "type": "string",
                        "enum": ["path", "content"],
                        "description": "path = filenames (default); content = notes, chats, image captions, audio transcripts"
                    },
                    "path": { "type": "string", "description": "Optional folder to scope, e.g. /notes" }
                },
                "required": ["query"]
            }),
        ),
        fn_tool(
            "create",
            "Create a note or folder. Trailing '/' makes a folder (and missing parents). Fails if the path exists. For file contents, create then edit.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "e.g. /notes/todo.md or /notes/" }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "rename",
            "Rename a note or folder (basename only). Use move to change folders.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "name": { "type": "string", "description": "New filename, no slashes" }
                },
                "required": ["path", "name"]
            }),
        ),
        fn_tool(
            "move",
            "Move a note or folder into another folder. `to` is the destination folder.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "to": { "type": "string", "description": "Destination folder, e.g. /notes or /notes/" }
                },
                "required": ["path", "to"]
            }),
        ),
        fn_tool(
            "delete",
            "Delete a note or folder (folders include their contents).",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "recent",
            "List recently used notes (activity, not filename dates).",
            json!({ "type": "object", "properties": {} }),
        ),
        fn_tool(
            "pin",
            "List pinned notes, or pin / unpin a document. Omit path to list. `.` is this conversation.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path, or `.` for this chat" },
                    "remove": {
                        "type": "boolean",
                        "description": "If true, unpin path instead of pinning it"
                    }
                }
            }),
        ),
        fn_tool(
            "duplicate",
            "Duplicate a document in the same folder (auto-named copy).",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "contacts",
            "List people this Lockbook has shared with or been shared with. Usernames only.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Optional filter; omit to list all"
                    }
                }
            }),
        ),
        fn_tool(
            "share",
            "Share a note, list pending shares, or accept / reject one. \
             New shares need a path and username. Accept needs id and a folder (`to`).",
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["new", "pending", "accept", "reject"],
                        "description": "Omit with path+username for new; omit with no args for pending"
                    },
                    "path": { "type": "string" },
                    "username": { "type": "string" },
                    "mode": {
                        "type": "string",
                        "enum": ["read", "write"],
                        "description": "Access for a new share; default write"
                    },
                    "id": { "type": "string", "description": "Pending share id (or unique name)" },
                    "to": { "type": "string", "description": "Folder to place an accepted share" }
                }
            }),
        ),
        fn_tool(
            "account",
            "Show Lockbook username, plan, storage usage, and sync status.",
            json!({ "type": "object", "properties": {} }),
        ),
        fn_tool(
            "now",
            "Current local date and time. Call when scheduling or referring to today, this week, or a deadline.",
            json!({ "type": "object", "properties": {} }),
        ),
        fn_tool(
            "tabs",
            "Workspace tabs: omit action to list (current marked, live = on a call). \
             open/focus a path (`.` is this chat), close a tab, move it \
             (to: start, end, left, right, next, prev, or a 1-based index), \
             or go back/forward in the current tab's history (or to the previous tab).",
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "open", "focus", "close", "move", "back", "forward"],
                        "description": "Omit with no path to list; omit with a path to open/focus"
                    },
                    "path": {
                        "type": "string",
                        "description": "File path, or `.` for this chat"
                    },
                    "to": {
                        "type": "string",
                        "description": "For move: start, end, left, right, next, prev, or 1-based index"
                    }
                }
            }),
        ),
        fn_tool(
            "hangup",
            "End the current voice call. The conversation continues as typed chat. No-op if not on a call.",
            json!({ "type": "object", "properties": {} }),
        ),
        fn_tool(
            "imagine",
            "Generate or edit an image into a Lockbook file. Omit path to write assets/imagine_<time>.png next to this chat. Pass from (path or list, max 5) to edit those images. Embed in a reply with markdown ![](path) or open with tabs.",
            json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "What to generate or how to edit" },
                    "path": { "type": "string", "description": "Destination file path. Created if missing." },
                    "from": {
                        "description": "Source image path, or up to 5 paths to combine",
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ]
                    },
                    "aspect": { "type": "string", "description": "e.g. 1:1, 16:9, 9:16. Omit for auto." },
                    "quality": { "type": "string", "enum": ["low", "medium", "auto"] }
                },
                "required": ["prompt"]
            }),
        ),
        fn_tool(
            "look",
            "Inspect a Lockbook image and report what's in it. jpeg/png; other formats are converted.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Image path" }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "download",
            "Download an http(s) URL into a Lockbook file. Omit path to write next to this chat under assets/. Images can be embedded with markdown ![](path).",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "http(s) URL" },
                    "path": { "type": "string", "description": "Destination file path. Created if missing." }
                },
                "required": ["url"]
            }),
        ),
        fn_tool(
            "transcribe",
            "Transcribe a Lockbook audio file. Word timings are saved inside the file (ID3). Returns the plain transcript.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Audio file path" }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "caption",
            "Get or set the caption stored inside a jpeg/png (PNG Description / JPEG comment). Omit text to read. Empty text clears. Does not run vision; use look to generate a caption.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Image path" },
                    "text": { "type": "string", "description": "Omit to read. Set to write. Empty to clear." }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "transcript",
            "Get or set the transcript stored inside an mp3 (ID3). Omit text to read. Empty text clears. Does not run speech-to-text; use transcribe to generate timings.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Audio path" },
                    "text": { "type": "string", "description": "Omit to read. Set to write. Empty to clear." }
                },
                "required": ["path"]
            }),
        ),
        fn_tool(
            "record",
            "Save text-to-speech as a Lockbook .mp3 file. Does not play on a voice call and does not capture the microphone. Only when the user wants an audio file. Omit path to write assets/record_<time>.mp3 next to this chat.",
            json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to record as audio" },
                    "path": { "type": "string", "description": "Destination .mp3 path" },
                    "voice": { "type": "string", "description": "Grok voice id, e.g. eve" }
                },
                "required": ["text"]
            }),
        ),
        fn_tool(
            "settings",
            "List or set this device's Grok voice, microphone, or speaker. \
             Omit name to list (current marked). Pass a name (exact or unique \
             substring) to select, or name=next to cycle. Persists across chats.",
            json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["input", "output", "voice"],
                        "description": "input = microphone, output = speaker, voice = Grok voice"
                    },
                    "name": {
                        "type": "string",
                        "description": "Omit to list. Device or voice name, or next"
                    }
                },
                "required": ["kind"]
            }),
        ),
    ])
}

pub fn summary(name: &str, args: &Value) -> String {
    let path = args.get("path").and_then(Value::as_str).unwrap_or("/");
    match name {
        "list" => format!("list {path}"),
        "read" => format!("read {path}"),
        "info" => format!("info {path}"),
        "this" => "this chat".into(),
        "edit" => format!("edit {path}"),
        "search" => {
            let q = args.get("query").and_then(Value::as_str).unwrap_or("");
            match args.get("in").and_then(Value::as_str) {
                Some("content") => format!("search {q} in contents"),
                _ => format!("search {q}"),
            }
        }
        "create" => format!("create {path}"),
        "rename" => {
            let name = args.get("name").and_then(Value::as_str).unwrap_or("");
            format!("rename {path} → {name}")
        }
        "move" => {
            let to = args.get("to").and_then(Value::as_str).unwrap_or("/");
            format!("move {path} → {to}")
        }
        "delete" => format!("delete {path}"),
        "recent" => "recent".into(),
        "pin" => {
            if args.get("remove").and_then(Value::as_bool) == Some(true) {
                format!("unpin {path}")
            } else if args.get("path").and_then(Value::as_str).is_some() {
                format!("pin {path}")
            } else {
                "pins".into()
            }
        }
        "duplicate" => format!("duplicate {path}"),
        "share" => share_summary(args),
        "contacts" => {
            let q = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if q.is_empty() { "contacts".into() } else { format!("contacts {q}") }
        }
        "account" => "account".into(),
        "now" => "now".into(),
        "hangup" => "hang up".into(),
        "imagine" => {
            let prompt = args.get("prompt").and_then(Value::as_str).unwrap_or("");
            let line = prompt.lines().next().unwrap_or("").trim();
            if path != "/" && !path.is_empty() {
                format!("imagine {path}")
            } else if line.is_empty() {
                "imagine".into()
            } else {
                format!("imagine {line}")
            }
        }
        "look" => format!("look {path}"),
        "download" => {
            if args.get("path").and_then(Value::as_str).is_some() && path != "/" {
                format!("download {path}")
            } else {
                format!("download {}", url_arg(args))
            }
        }
        "caption" => {
            if args.get("text").and_then(Value::as_str).is_some() {
                format!("caption {path}")
            } else {
                format!("read caption {path}")
            }
        }
        "transcript" => {
            if args.get("text").and_then(Value::as_str).is_some() {
                format!("transcript {path}")
            } else {
                format!("read transcript {path}")
            }
        }
        "transcribe" => format!("transcribe {path}"),
        "record" => {
            if path != "/" && args.get("path").and_then(Value::as_str).is_some() {
                format!("record {path}")
            } else {
                "record".into()
            }
        }
        "tabs" => tabs_summary(args),
        "settings" => {
            let kind = args
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("settings");
            match args
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(n) => format!("{kind} {n}"),
                None => format!("list {kind}"),
            }
        }
        "web_search" | "web_search_with_snippets" | "web_search_call" => {
            format!("web {}", query_arg(args))
        }
        "browse_page" | "open_page" | "open_page_with_find" => {
            format!("open {}", url_arg(args))
        }
        "x_search" | "x_keyword_search" | "x_semantic_search" | "x_search_call" => {
            format!("x {}", query_arg(args))
        }
        "x_user_search" => format!("x user {}", query_arg(args)),
        "x_thread_fetch" => "x thread".into(),
        "search_images" => format!("images {}", query_arg(args)),
        "view_image" | "view_x_video" => name.into(),
        "code_interpreter" | "code_interpreter_call" | "code_execution" | "code_execution_call" => {
            let code = args
                .get("code")
                .or_else(|| args.get("input"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .trim();
            if code.is_empty() { "code".into() } else { format!("code {code}") }
        }
        other => other.into(),
    }
}

fn query_arg(args: &Value) -> String {
    args.get("query")
        .or_else(|| args.get("q"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn url_arg(args: &Value) -> String {
    args.get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Body for a provider-run search row (no tool result comes back).
pub fn server_body(name: &str, args: &Value) -> String {
    let q = query_arg(args);
    let url = url_arg(args);
    if !q.is_empty() {
        format!("{name}: {q}")
    } else if !url.is_empty() {
        format!("{name}: {url}")
    } else if let Some(code) = args
        .get("code")
        .or_else(|| args.get("input"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let line = code.lines().next().unwrap_or("").trim();
        format!("{name}: {line}")
    } else {
        name.into()
    }
}

/// What we persist on a server-tool row: stdout / source URLs, not the
/// one-line title. The program and query live in `args`.
pub fn persist_body(name: &str, args: &Value) -> String {
    if is_code_name(name) {
        return code_logs(args);
    }
    source_entries(args)
        .into_iter()
        .map(|(_, url)| url)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Markdown for the expanded tool row. Empty means the title is the whole story.
pub fn expand_md(name: &str, args: Option<&Value>, result: &str) -> String {
    let empty = json!({});
    let args = args.unwrap_or(&empty);
    let result = result.trim();
    let result = if is_server(name) && is_echo_result(name, args, result) { "" } else { result };

    if is_code_name(name) {
        let code = code_src(args);
        let logs = {
            let logs = code_logs(args);
            if logs.is_empty() { result.to_string() } else { logs }
        };
        let mut s = String::new();
        if !code.is_empty() {
            s.push_str(&fence("python", &clip_detail(&code)));
        }
        if !logs.is_empty() {
            if !s.is_empty() {
                s.push_str("\n\n");
            }
            s.push_str(&fence("", &clip_detail(&logs)));
        }
        return s;
    }

    let sources = source_entries(args);
    if !sources.is_empty() {
        return sources
            .into_iter()
            .take(SEARCH_CAP)
            .map(|(title, url)| {
                if title.is_empty() || title == url {
                    format!("- {url}")
                } else {
                    format!("- [{title}]({url})")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    if is_server(name) {
        let urls: Vec<&str> = result
            .lines()
            .map(str::trim)
            .filter(|l| looks_like_url(l))
            .collect();
        if !urls.is_empty() {
            return urls
                .into_iter()
                .take(SEARCH_CAP)
                .map(|u| format!("- {u}"))
                .collect::<Vec<_>>()
                .join("\n");
        }
        if result.is_empty() {
            return String::new();
        }
    }

    if result.is_empty() {
        return String::new();
    }
    if result.contains('\n') || result.len() > 80 {
        fence("", &clip_detail(result))
    } else {
        result.to_string()
    }
}

fn is_code_name(name: &str) -> bool {
    matches!(
        name,
        "code_interpreter" | "code_interpreter_call" | "code_execution" | "code_execution_call"
    ) || name.starts_with("code_")
}

fn is_echo_result(name: &str, args: &Value, result: &str) -> bool {
    if result.is_empty() {
        return true;
    }
    if result == summary(name, args) || result == server_body(name, args) {
        return true;
    }
    result.split_once(':').is_some_and(|(head, _)| head == name)
}

fn code_src(args: &Value) -> String {
    args.get("code")
        .or_else(|| args.get("input"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn code_logs(args: &Value) -> String {
    let Some(outputs) = args.get("outputs") else {
        return String::new();
    };
    if let Some(s) = outputs.as_str() {
        return s.to_string();
    }
    let Some(arr) = outputs.as_array() else {
        return String::new();
    };
    let mut parts = Vec::new();
    for o in arr {
        if let Some(logs) = o.get("logs").and_then(Value::as_str) {
            parts.push(logs.to_string());
        } else if o.get("type").and_then(Value::as_str) == Some("image") {
            continue;
        } else if let Some(text) = o.get("text").and_then(Value::as_str) {
            parts.push(text.to_string());
        } else if let Some(s) = o.as_str() {
            parts.push(s.to_string());
        }
    }
    parts.join("\n")
}

fn source_entries(args: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for key in ["sources", "citations", "results"] {
        push_sources(&mut out, args.get(key));
    }
    out
}

fn push_sources(out: &mut Vec<(String, String)>, v: Option<&Value>) {
    let Some(v) = v else {
        return;
    };
    match v {
        Value::Array(arr) => {
            for x in arr {
                push_source(out, x);
            }
        }
        other => push_source(out, other),
    }
}

fn push_source(out: &mut Vec<(String, String)>, v: &Value) {
    let url = v
        .get("url")
        .or_else(|| v.get("uri"))
        .or_else(|| v.get("href"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| {
            v.as_str()
                .filter(|s| looks_like_url(s))
                .map(|s| s.to_string())
        });
    let Some(url) = url else {
        return;
    };
    if out.iter().any(|(_, u)| u == &url) {
        return;
    }
    let title = v
        .get("title")
        .or_else(|| v.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    out.push((title, url));
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn clip_detail(s: &str) -> String {
    const MAX_LINES: usize = 80;
    const MAX_CHARS: usize = 8_000;
    let mut out = String::new();
    for (i, line) in s.lines().enumerate() {
        if i >= MAX_LINES || out.len() >= MAX_CHARS {
            out.push_str("\n…");
            break;
        }
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

fn fence(lang: &str, body: &str) -> String {
    let mut run = 0usize;
    let mut best = 0usize;
    for c in body.chars() {
        if c == '`' {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    let ticks = "`".repeat(best.max(2) + 1);
    if lang.is_empty() {
        format!("{ticks}\n{body}\n{ticks}")
    } else {
        format!("{ticks}{lang}\n{body}\n{ticks}")
    }
}

fn now() -> String {
    format_now(chrono::Local::now())
}

fn format_now<Tz: chrono::TimeZone>(t: chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    t.format("%A %Y-%m-%d %H:%M %z").to_string()
}

pub fn run(core: &Lb, chat_id: lb_rs::Uuid, name: &str, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    info!(tool = name, %path, "chat tool run");
    let result = match name {
        "list" => list(core, chat_id, args),
        "read" => read(core, chat_id, args),
        "info" => info(core, chat_id, args),
        "this" => this_chat(core, chat_id),
        "edit" => edit(core, chat_id, args),
        "search" => search(core, chat_id, args),
        "create" => create(core, args),
        "rename" => rename(core, chat_id, args),
        "move" => move_file(core, chat_id, args),
        "delete" => delete(core, chat_id, args),
        "recent" => recent(core, chat_id),
        "pin" => pin(core, chat_id, args),
        "duplicate" => duplicate(core, chat_id, args),
        "share" => share(core, chat_id, args),
        "contacts" => contacts(core, args),
        "account" => account(core),
        "now" => Ok(now()),
        "hangup" => Ok("not on a call".into()),
        "caption" => caption(core, chat_id, args),
        "transcript" => transcript(core, chat_id, args),
        "imagine" | "look" | "transcribe" | "record" => {
            Err(format!("{name} needs Grok auth; dispatched by chat"))
        }
        "download" => Err("download is dispatched by chat".into()),
        other => Err(format!("unknown tool {other}")),
    };
    if let Err(e) = &result {
        warn!(tool = name, %path, error = %e, "chat tool failed");
    }
    result
}

fn path_arg(args: &Value) -> String {
    let p = args.get("path").and_then(Value::as_str).unwrap_or("/");
    if p.is_empty() { "/".into() } else { p.to_string() }
}

fn hide_dotted(listed: &str, child: &str) -> bool {
    let grant = path_segments(listed);
    let path = path_segments(child);
    path.get(grant.len()..)
        .is_some_and(|rest| rest.iter().any(|s| s.starts_with('.')))
}

/// True when any path segment is a `.`-prefixed name. `.` alone is this chat.
fn is_hidden(path: &str) -> bool {
    if path == "." {
        return false;
    }
    path_segments(path).iter().any(|s| s.starts_with('.'))
}

pub(super) fn refuse_hidden(path: &str) -> Result<(), String> {
    if is_hidden(path) {
        Err("can't use hidden paths (names starting with .)".into())
    } else {
        Ok(())
    }
}

pub(super) fn open_file(
    core: &Lb, chat_id: lb_rs::Uuid, path: &str,
) -> Result<lb_rs::model::file::File, String> {
    if path == "." {
        return core.get_file_by_id(chat_id).map_err(|e| e.to_string());
    }
    refuse_hidden(path)?;
    if path.is_empty() || path == "/" {
        return Err("needs a file path".into());
    }
    core.get_by_path(path)
        .map_err(|e| format!("couldn't open {path}: {e}"))
}

fn list(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    let folder = if path == "/" {
        refuse_hidden(&path)?;
        core.get_root().map_err(|e| e.to_string())?
    } else {
        open_file(core, chat_id, &path)?
    };
    if folder.file_type != FileType::Folder {
        return Err(format!("{path} is not a folder"));
    }
    let mut names: Vec<String> = core
        .get_children(&folder.id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|f| {
            let child = if path == "/" {
                format!("/{}", f.name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), f.name)
            };
            if hide_dotted(&path, &child) {
                return None;
            }
            Some(if f.file_type == FileType::Folder { format!("{}/", f.name) } else { f.name })
        })
        .collect();
    names.sort();
    Ok(format_list_names(names))
}

fn read(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    if path == "/" {
        return Err("read needs a file path".into());
    }
    let file = open_file(core, chat_id, &path)?;
    if file.file_type == FileType::Folder {
        return Err(format!("{path} is a folder; list it instead"));
    }
    let bytes = core
        .read_document(file.id, false)
        .map_err(|e| e.to_string())?;
    if bytes.len() > READ_CAP {
        return Err(format!(
            "file is {} bytes (cap {READ_CAP}). Search for the section, or split the note.",
            bytes.len()
        ));
    }
    String::from_utf8(bytes).map_err(|_| "not UTF-8".to_string())
}

fn info(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    let file = if path == "/" {
        refuse_hidden(&path)?;
        core.get_root().map_err(|e| e.to_string())?
    } else {
        open_file(core, chat_id, &path)?
    };
    let shown = if path == "/" {
        "/".into()
    } else {
        file_path(core, file.id).unwrap_or_else(|_| path.clone())
    };
    let kind = match file.file_type {
        FileType::Folder => "folder",
        FileType::Document => "document",
        FileType::Link { .. } => "link",
    };
    let modified = core.get_timestamp_human_string(file.last_modified as i64);
    let mut lines = vec![shown, format!("type: {kind}")];
    match file.file_type {
        FileType::Folder => {
            let n = core.get_children(&file.id).map(|c| c.len()).unwrap_or(0);
            lines.push(format!("items: {n}"));
        }
        FileType::Link { target } => {
            if let Ok(t) = file_path(core, target) {
                lines.push(format!("target: {t}"));
            }
            lines.push(format!("size: {}", bytes_to_human(file.size_bytes)));
        }
        FileType::Document => {
            lines.push(format!("size: {}", bytes_to_human(file.size_bytes)));
        }
    }
    if file.file_type == FileType::Document {
        let pinned = core
            .list_pinned()
            .map(|ids| ids.contains(&file.id))
            .unwrap_or(false);
        lines.push(format!("pinned: {}", if pinned { "yes" } else { "no" }));
    }
    lines.push(format!("modified: {modified}"));
    if !file.last_modified_by.is_empty() {
        lines.push(format!("modified by: {}", file.last_modified_by));
    }
    if !file.owner.is_empty() {
        lines.push(format!("owner: {}", file.owner));
    }
    if !file.shares.is_empty() {
        let shares: Vec<String> = file
            .shares
            .iter()
            .map(|s| {
                let mode = match s.mode {
                    ShareMode::Read => "read",
                    ShareMode::Write => "write",
                };
                format!("{} ({mode})", s.shared_with)
            })
            .collect();
        lines.push(format!("shared with: {}", shares.join(", ")));
    }
    Ok(lines.join("\n"))
}

fn this_chat(core: &Lb, chat_id: lb_rs::Uuid) -> Result<String, String> {
    info(core, chat_id, &json!({ "path": "." }))
}

fn tabs_summary(args: &Value) -> String {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("");
    let path = args.get("path").and_then(Value::as_str).unwrap_or("");
    let to = args.get("to").and_then(Value::as_str).unwrap_or("");
    match action {
        "close" => format!("close {path}"),
        "move" => format!("move tab {path} → {to}"),
        "open" | "focus" => format!("show {path}"),
        "back" => "back".into(),
        "forward" => "forward".into(),
        "list" => "tabs".into(),
        _ if !path.is_empty() => format!("show {path}"),
        _ => "tabs".into(),
    }
}

/// List / open / close / reorder workspace tabs. Ops are applied by the host.
pub fn tabs(
    core: &Lb, chat_id: lb_rs::Uuid, args: &Value, snaps: &[TabSnap],
) -> Result<(String, Vec<TabOp>), String> {
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let to = args.get("to").and_then(Value::as_str).unwrap_or("").trim();
    match action {
        "" | "list" if path.is_empty() && to.is_empty() => Ok((format_tab_list(snaps), Vec::new())),
        "close" => {
            let id = tab_file(core, chat_id, path)?;
            if id == chat_id {
                return Err("can't close this chat from here".into());
            }
            let shown = tab_path(snaps, id, path);
            Ok((format!("closing {shown}"), vec![TabOp::Close(id)]))
        }
        "move" => {
            let id = tab_file(core, chat_id, path)?;
            let from = snaps
                .iter()
                .position(|s| s.file_id == id)
                .ok_or_else(|| format!("{path} is not open; open it first"))?;
            let dest = parse_move_to(to, from, snaps.len())?;
            let shown = tab_path(snaps, id, path);
            if dest == from {
                return Ok((format!("{shown} is already there"), Vec::new()));
            }
            Ok((format!("moving {shown} to {}", dest + 1), vec![TabOp::Move { id, to: dest }]))
        }
        "open" | "focus" | "" if !path.is_empty() => {
            let id = tab_file(core, chat_id, path)?;
            let file = core.get_file_by_id(id).map_err(|e| e.to_string())?;
            if file.file_type == FileType::Folder {
                return Err(format!("{path} is a folder; open a file"));
            }
            let shown = tab_path(snaps, id, path);
            Ok((format!("showing {shown}"), vec![TabOp::Open(id)]))
        }
        "back" | "forward" => tab_history_op(action, snaps),
        _ => Err("tabs needs action=list, open, focus, close, move, back, or forward".into()),
    }
}

fn tab_history_op(action: &str, snaps: &[TabSnap]) -> Result<(String, Vec<TabOp>), String> {
    let cur = snaps
        .iter()
        .find(|s| s.active)
        .ok_or_else(|| "no tabs".to_string())?;
    match action {
        "back" => {
            if !cur.can_back {
                return Err("nothing to go back to".into());
            }
            Ok(("going back".into(), vec![TabOp::Back]))
        }
        "forward" => {
            if !cur.can_forward {
                return Err("nothing to go forward to".into());
            }
            Ok(("going forward".into(), vec![TabOp::Forward]))
        }
        _ => Err("tabs needs action=list, open, focus, close, move, back, or forward".into()),
    }
}

fn tab_file(core: &Lb, chat_id: lb_rs::Uuid, path: &str) -> Result<lb_rs::Uuid, String> {
    if path.is_empty() {
        return Err("needs a path (`.` is this chat)".into());
    }
    Ok(open_file(core, chat_id, path)?.id)
}

fn tab_path(snaps: &[TabSnap], id: lb_rs::Uuid, fallback: &str) -> String {
    snaps
        .iter()
        .find(|s| s.file_id == id)
        .map(|s| s.path.clone())
        .unwrap_or_else(|| if fallback == "." { "this chat".into() } else { fallback.to_string() })
}

fn format_tab_list(snaps: &[TabSnap]) -> String {
    if snaps.is_empty() {
        return "no open tabs".into();
    }
    let current = snaps
        .iter()
        .find(|s| s.active)
        .map(|s| s.path.as_str())
        .unwrap_or("none");
    let mut s = format!("tabs (current {current}):\n");
    for (i, t) in snaps.iter().enumerate() {
        s.push_str(&format!("{}. {}", i + 1, t.path));
        if t.active {
            s.push_str("  [current]");
        }
        if t.live {
            s.push_str("  [live]");
        }
        s.push('\n');
    }
    s.pop();
    s
}

fn parse_move_to(to: &str, from: usize, n: usize) -> Result<usize, String> {
    if n == 0 {
        return Err("no tabs".into());
    }
    let last = n - 1;
    let dest = match to {
        "" => return Err("move needs to=start, end, left, right, or a 1-based index".into()),
        "start" | "begin" | "first" => 0,
        "end" | "last" => last,
        "left" | "prev" | "previous" => from.saturating_sub(1),
        "right" | "next" => (from + 1).min(last),
        _ => {
            let k: usize = to
                .parse()
                .map_err(|_| format!("unknown move target {to:?}"))?;
            if k == 0 {
                return Err("tabs are numbered from 1".into());
            }
            let i = k - 1;
            if i > last {
                return Err(format!("only {n} tabs"));
            }
            i
        }
    };
    Ok(dest)
}

fn edit(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    let old = args.get("old").and_then(Value::as_str).unwrap_or("");
    let new = args.get("new").and_then(Value::as_str).unwrap_or("");
    if path == "/" {
        return Err("edit needs a file path".into());
    }
    refuse_hidden(&path)?;
    if old.is_empty() {
        match open_file(core, chat_id, &path) {
            Ok(file) => {
                core.write_document(file.id, new.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            Err(_) => {
                core.create_at_path(&path)
                    .map_err(|e| format!("couldn't create {path}: {e}"))?;
                let file = core.get_by_path(&path).map_err(|e| e.to_string())?;
                core.write_document(file.id, new.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
        }
        return Ok(format!("wrote {path} ({} bytes)", new.len()));
    }
    let file = open_file(core, chat_id, &path)?;
    let bytes = core
        .read_document(file.id, false)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "not UTF-8".to_string())?;
    let count = text.matches(old).count();
    if count != 1 {
        return Err(format!(
            "old occurs {count} times; need exactly once. Copy a unique nearby sentence."
        ));
    }
    let next = text.replacen(old, new, 1);
    core.write_document(file.id, next.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(format!("edited {path}"))
}

fn create(core: &Lb, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    if path == "/" {
        return Err("create needs a path".into());
    }
    refuse_hidden(&path)?;
    if core.get_by_path(&path).is_ok() {
        return Err(format!("{path} already exists"));
    }
    core.create_at_path(&path)
        .map_err(|e| format!("couldn't create {path}: {e}"))?;
    Ok(format!("created {path}"))
}

fn rename(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    let name = args
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if path == "/" {
        return Err("can't rename the root".into());
    }
    if let Err(e) = basename(name) {
        return Err(e);
    }
    if name.starts_with('.') {
        return Err("can't rename to a hidden name".into());
    }
    let file = open_file(core, chat_id, &path)?;
    core.rename_file(&file.id, name)
        .map_err(|e| format!("couldn't rename {path}: {e}"))?;
    Ok(format!("renamed {path} to {name}"))
}

fn move_file(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    let to = args.get("to").and_then(Value::as_str).unwrap_or("").trim();
    if path == "/" {
        return Err("can't move the root".into());
    }
    if to.is_empty() {
        return Err("move needs a destination folder".into());
    }
    let file = open_file(core, chat_id, &path)?;
    let dest = if to == "/" {
        core.get_root().map_err(|e| e.to_string())?
    } else {
        open_file(core, chat_id, to)?
    };
    if dest.file_type != FileType::Folder {
        return Err(format!("{to} is not a folder"));
    }
    if dest.id == file.id {
        return Err("can't move a folder into itself".into());
    }
    core.move_file(&file.id, &dest.id)
        .map_err(|e| format!("couldn't move {path}: {e}"))?;
    Ok(format!("moved {path} to {to}"))
}

fn delete(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    if path == "/" {
        return Err("can't delete the root".into());
    }
    let file = open_file(core, chat_id, &path)?;
    core.delete_file(&file.id)
        .map_err(|e| format!("couldn't delete {path}: {e}"))?;
    Ok(format!("deleted {path}"))
}

fn caption(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    refuse_hidden(&path)?;
    let file = open_file(core, chat_id, &path)?;
    if file.file_type == FileType::Folder {
        return Err(format!("{path} is a folder"));
    }
    let bytes = core
        .read_document(file.id, false)
        .map_err(|e| format!("couldn't read {path}: {e}"))?;
    match args.get("text").and_then(Value::as_str) {
        None => {
            Ok(media_text::get_caption(&file.name, &bytes).unwrap_or_else(|| "(no caption)".into()))
        }
        Some(text) => {
            let out = media_text::set_caption(&file.name, &bytes, text)?;
            core.write_document(file.id, &out)
                .map_err(|e| format!("couldn't write {path}: {e}"))?;
            if text.is_empty() {
                Ok(format!("cleared caption on {path}"))
            } else {
                Ok(format!("set caption on {path}"))
            }
        }
    }
}

fn transcript(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    refuse_hidden(&path)?;
    let file = open_file(core, chat_id, &path)?;
    if file.file_type == FileType::Folder {
        return Err(format!("{path} is a folder"));
    }
    let bytes = core
        .read_document(file.id, false)
        .map_err(|e| format!("couldn't read {path}: {e}"))?;
    if !media_text::looks_like_mp3(&file.name, &bytes) {
        return Err(format!("{path} is not an mp3"));
    }
    match args.get("text").and_then(Value::as_str) {
        None => {
            let t = media_text::extract(&bytes);
            match t {
                Some(t) if !t.text.is_empty() => Ok(t.text),
                _ => Ok("(no transcript)".into()),
            }
        }
        Some(text) => {
            let out = media_text::embed(&bytes, &media_text::Transcript::from_text(text))?;
            core.write_document(file.id, &out)
                .map_err(|e| format!("couldn't write {path}: {e}"))?;
            if text.is_empty() {
                Ok(format!("cleared transcript on {path}"))
            } else {
                Ok(format!("set transcript on {path}"))
            }
        }
    }
}

fn basename(name: &str) -> Result<(), String> {
    if name.is_empty() {
        Err("rename needs a new name".into())
    } else if name.contains('/') {
        Err("name is a basename, not a path; use move to change folders".into())
    } else {
        Ok(())
    }
}

fn search(core: &Lb, _chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if query.is_empty() {
        return Err("search needs a query".into());
    }
    let scope = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|p| !p.is_empty() && *p != "/");
    if let Some(p) = scope {
        refuse_hidden(p)?;
    }
    let filter = scope.map(|p| SearchFilter::Path(p.to_string()));
    let content = args.get("in").and_then(Value::as_str) == Some("content");
    if content {
        let mut s = core.content_searcher();
        if let Some(f) = filter {
            s.update_filter(Some(f));
        }
        s.query(query);
        Ok(format_content_hits(&s))
    } else {
        let mut s = core.path_searcher();
        if let Some(f) = filter {
            s.update_filter(Some(f));
        }
        s.query(query);
        Ok(format_search_hits(s.results()))
    }
}

fn format_list_names(mut names: Vec<String>) -> String {
    if names.is_empty() {
        return "(empty)".into();
    }
    if names.len() > LIST_CAP {
        let extra = names.len() - LIST_CAP;
        names.truncate(LIST_CAP);
        names.push(format!("…and {extra} more; search this folder instead of listing again"));
    }
    names.join("\n")
}

fn search_path(r: &SearchResult) -> String {
    let path = if r.parent_path.is_empty() || r.parent_path == "/" {
        format!("/{}", r.filename)
    } else {
        format!("{}/{}", r.parent_path.trim_end_matches('/'), r.filename)
    };
    if r.is_folder { format!("{path}/") } else { path }
}

fn format_search_hits(hits: &[SearchResult]) -> String {
    let mut lines = Vec::new();
    let mut extra = 0usize;
    for r in hits {
        let path = search_path(r);
        if hide_dotted("/", &path) {
            continue;
        }
        if lines.len() >= SEARCH_CAP {
            extra += 1;
            continue;
        }
        lines.push(path);
    }
    if lines.is_empty() {
        return "no matches".into();
    }
    if extra > 0 {
        lines.push(format!("…and {extra} more"));
    }
    lines.join("\n")
}

fn format_content_hits(s: &lb_rs::search::ContentSearcher) -> String {
    let mut lines = Vec::new();
    let mut kept = 0usize;
    let mut extra = 0usize;
    for r in s.results() {
        let path = search_path(r);
        if hide_dotted("/", &path) {
            continue;
        }
        if kept >= SEARCH_CAP {
            extra += 1;
            continue;
        }
        kept += 1;
        lines.push(path);
        if let Some(m) = r.content_matches.first() {
            if let Some((pre, mat, suf)) = s.snippet(r.id, &m.range, SNIPPET_CONTEXT) {
                let snip = flatten_snippet(pre, mat, suf);
                if !snip.is_empty() {
                    lines.push(format!("  {snip}"));
                }
            }
        }
    }
    if lines.is_empty() {
        return "no matches".into();
    }
    if extra > 0 {
        lines.push(format!("…and {extra} more"));
    }
    lines.join("\n")
}

fn flatten_snippet(prefix: &str, matched: &str, suffix: &str) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect()
    };
    let pre = clean(prefix);
    let mat = clean(matched);
    let suf = clean(suffix);
    let mut out = String::new();
    if !pre.is_empty() {
        out.push('…');
        out.push_str(&pre);
    }
    out.push_str(&mat);
    out.push_str(&suf);
    if !suf.is_empty() {
        out.push('…');
    }
    let chars: Vec<char> = out.chars().collect();
    if chars.len() > SNIPPET_CAP {
        let mut n = SNIPPET_CAP.saturating_sub(1);
        while n > 0 && chars[n] != ' ' {
            n -= 1;
        }
        if n == 0 {
            n = SNIPPET_CAP.saturating_sub(1);
        }
        let mut s: String = chars[..n].iter().collect();
        s.push('…');
        return s;
    }
    out
}

fn share_summary(args: &Value) -> String {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("");
    let path = args.get("path").and_then(Value::as_str).unwrap_or("");
    let user = args.get("username").and_then(Value::as_str).unwrap_or("");
    match action {
        "pending" | "" if path.is_empty() && user.is_empty() => "pending shares".into(),
        "accept" => "accept share".into(),
        "reject" => "reject share".into(),
        _ if !user.is_empty() => format!("share {path} with {user}"),
        _ => "share".into(),
    }
}

fn file_path(core: &Lb, id: lb_rs::Uuid) -> Result<String, String> {
    core.get_path_by_id(id).map_err(|e| e.to_string())
}

fn resolve_path(
    core: &Lb, chat_id: lb_rs::Uuid, path: &str,
) -> Result<lb_rs::model::file::File, String> {
    open_file(core, chat_id, path)
}

fn recent(core: &Lb, _chat_id: lb_rs::Uuid) -> Result<String, String> {
    let ids = core
        .suggested_docs(RankingWeights::default())
        .map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    for id in ids {
        match file_path(core, id) {
            Ok(p) if !is_hidden(&p) => lines.push(p),
            _ => continue,
        }
    }
    if lines.is_empty() { Ok("no recent notes".into()) } else { Ok(lines.join("\n")) }
}

fn pin(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let remove = args.get("remove").and_then(Value::as_bool) == Some(true);
    if path.is_empty() {
        let ids = core.list_pinned().map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for id in ids {
            if let Ok(p) = file_path(core, id) {
                if !is_hidden(&p) {
                    lines.push(p);
                }
            }
        }
        return if lines.is_empty() { Ok("no pinned notes".into()) } else { Ok(lines.join("\n")) };
    }
    let file = resolve_path(core, chat_id, path)?;
    if remove {
        core.unpin_file(file.id).map_err(|e| e.to_string())?;
        Ok(format!("unpinned {path}"))
    } else {
        core.pin_file(file.id).map_err(|e| e.to_string())?;
        Ok(format!("pinned {path}"))
    }
}

fn duplicate(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let path = path_arg(args);
    let file = resolve_path(core, chat_id, &path)?;
    let copy = core
        .duplicate_file(&file.id)
        .map_err(|e| format!("couldn't duplicate {path}: {e}"))?;
    let new_path = file_path(core, copy.id).unwrap_or_else(|_| copy.name.clone());
    Ok(format!("duplicated {path} → {new_path}"))
}

fn share(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<String, String> {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("");
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let username = args
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    match action {
        "pending" => pending_shares(core),
        "accept" => accept_share(core, args),
        "reject" => reject_share(core, args),
        "new" => share_new(core, chat_id, path, username, args),
        "" if !path.is_empty() && !username.is_empty() => {
            share_new(core, chat_id, path, username, args)
        }
        "" if path.is_empty() && username.is_empty() => pending_shares(core),
        _ => Err("share needs action=new, pending, accept, or reject".into()),
    }
}

fn share_new(
    core: &Lb, chat_id: lb_rs::Uuid, path: &str, username: &str, args: &Value,
) -> Result<String, String> {
    if username.is_empty() {
        return Err("share needs a username".into());
    }
    let file = resolve_path(core, chat_id, path)?;
    let mode = match args.get("mode").and_then(Value::as_str).unwrap_or("write") {
        "read" | "read-only" | "ro" => ShareMode::Read,
        _ => ShareMode::Write,
    };
    core.share_file(file.id, username, mode)
        .map_err(|e| format!("couldn't share {path}: {e}"))?;
    let access = match mode {
        ShareMode::Read => "read",
        ShareMode::Write => "write",
    };
    Ok(format!("shared {path} with {username} ({access}); sync to send"))
}

fn pending_shares(core: &Lb) -> Result<String, String> {
    let pending = core.get_pending_shares().map_err(|e| e.to_string())?;
    if pending.is_empty() {
        return Ok("no pending shares".into());
    }
    let mut lines = Vec::new();
    for f in pending {
        let (from, mode) = f
            .shares
            .first()
            .map(|s| (s.shared_by.as_str(), s.mode))
            .unwrap_or(("", ShareMode::Write));
        lines.push(format!("{}  {}  from {from}  ({mode})", f.id, f.name));
    }
    Ok(lines.join("\n"))
}

fn find_pending(core: &Lb, spec: &str) -> Result<lb_rs::model::file::File, String> {
    let pending = core.get_pending_shares().map_err(|e| e.to_string())?;
    if spec.is_empty() {
        return Err("needs a pending share id".into());
    }
    if let Ok(id) = spec.parse::<lb_rs::Uuid>() {
        return pending
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| format!("no pending share {spec}"));
    }
    let hits: Vec<_> = pending
        .into_iter()
        .filter(|f| f.name.eq_ignore_ascii_case(spec))
        .collect();
    match hits.len() {
        1 => Ok(hits.into_iter().next().unwrap()),
        0 => Err(format!("no pending share named {spec}")),
        _ => Err(format!("several pending shares named {spec}; pass the id")),
    }
}

fn accept_share(core: &Lb, args: &Value) -> Result<String, String> {
    let spec = args
        .get("id")
        .or_else(|| args.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let to = args.get("to").and_then(Value::as_str).unwrap_or("").trim();
    if to.is_empty() {
        return Err("accept needs a destination folder (`to`)".into());
    }
    let share = find_pending(core, spec)?;
    let parent = if to == "/" {
        core.get_root().map_err(|e| e.to_string())?
    } else {
        core.get_by_path(to)
            .map_err(|e| format!("couldn't open {to}: {e}"))?
    };
    if parent.file_type != FileType::Folder {
        return Err(format!("{to} is not a folder"));
    }
    core.create_file(&share.name, &parent.id, FileType::Link { target: share.id })
        .map_err(|e| format!("couldn't accept share: {e}"))?;
    Ok(format!("accepted {} into {to}", share.name))
}

fn reject_share(core: &Lb, args: &Value) -> Result<String, String> {
    let spec = args
        .get("id")
        .or_else(|| args.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let share = find_pending(core, spec)?;
    core.delete_pending_share(&share.id)
        .map_err(|e| e.to_string())?;
    Ok(format!("rejected {}", share.name))
}

fn contacts(core: &Lb, args: &Value) -> Result<String, String> {
    let me = core.get_account().map(|a| a.username).unwrap_or_default();
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let mut names: Vec<String> = core
        .known_usernames()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|n| !n.eq_ignore_ascii_case(&me))
        .filter(|n| query.is_empty() || n.to_lowercase().contains(&query))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    if names.is_empty() {
        return Ok(if query.is_empty() {
            "no contacts yet".into()
        } else {
            format!("no contacts matching {query}")
        });
    }
    if names.len() > LIST_CAP {
        let extra = names.len() - LIST_CAP;
        names.truncate(LIST_CAP);
        names.push(format!("…and {extra} more"));
    }
    Ok(names.join("\n"))
}

fn account(core: &Lb) -> Result<String, String> {
    let acct = core.get_account().map_err(|e| e.to_string())?;
    let mut lines = vec![format!("username: {}", acct.username)];
    if let Ok(synced) = core.get_last_synced_human_string() {
        lines.push(format!("files last synced: {synced}"));
    }
    match core.get_local_changes() {
        Ok(c) if c.is_empty() => lines.push("unsynced changes: none".into()),
        Ok(c) if c.len() == 1 => lines.push("unsynced changes: 1 file".into()),
        Ok(c) => lines.push(format!("unsynced changes: {} files", c.len())),
        Err(_) => {}
    }
    match core.get_subscription_info() {
        Ok(Some(info)) => {
            use lb_rs::model::api::PaymentPlatform;
            match info.payment_platform {
                PaymentPlatform::Stripe { card_last_4_digits } => {
                    lines.push(format!("plan: Stripe *{card_last_4_digits}"));
                }
                PaymentPlatform::GooglePlay { .. } => lines.push("plan: Google Play".into()),
                PaymentPlatform::AppStore { .. } => lines.push("plan: App Store".into()),
            }
        }
        Ok(None) => lines.push("plan: trial".into()),
        Err(e) => lines.push(format!("plan: ({e})")),
    }
    match core.get_usage() {
        Ok(cap) => {
            let pct = if cap.data_cap.exact == 0 {
                0
            } else {
                (cap.server_usage.exact * 100) / cap.data_cap.exact
            };
            lines.push(format!(
                "usage: {} / {} ({}%)",
                cap.server_usage.readable, cap.data_cap.readable, pct
            ));
        }
        Err(e) => lines.push(format!("usage: ({e})")),
    }
    Ok(lines.join("\n"))
}

/// List or set voice / input / output. Mutates `state`; caller persists and
/// applies live (call duplex / session.update) when those exist.
pub fn settings(args: &Value, state: &mut SettingsState) -> Result<String, String> {
    let kind = args
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let spec = args
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match kind {
        "voice" => {
            let names: Vec<String> = VOICES.iter().map(|s| (*s).to_string()).collect();
            let current = if state.voice.is_empty() { DEFAULT_VOICE } else { state.voice.as_str() };
            match spec {
                None => Ok(format_choices("voice", &names, current)),
                Some(s) => {
                    let v = pick_named(&names, current, s)?;
                    state.voice = v.clone();
                    Ok(format!("voice is {v}"))
                }
            }
        }
        "input" => set_device("input", spec, &mut state.input),
        "output" => set_device("output", spec, &mut state.output),
        _ => Err("settings needs kind=input, output, or voice".into()),
    }
}

fn set_device(kind: &str, spec: Option<&str>, current: &mut String) -> Result<String, String> {
    let names = device_names(kind)?;
    let shown = if current.is_empty() { "default" } else { current.as_str() };
    match spec {
        None => Ok(format_choices(kind, &names, shown)),
        Some(s) => {
            let v = pick_named(&names, shown, s)?;
            *current = v.clone();
            Ok(format!("{kind} is {v}"))
        }
    }
}

fn device_names(kind: &str) -> Result<Vec<String>, String> {
    #[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
    {
        match kind {
            "input" => super::audio::Duplex::input_names(),
            "output" => super::audio::Duplex::output_names(),
            _ => Err(format!("unknown kind {kind}")),
        }
    }
    #[cfg(not(all(not(target_family = "wasm"), not(target_os = "android"))))]
    {
        let _ = kind;
        Err("audio devices are desktop-only".into())
    }
}

/// Exact match, unique substring, or `next` (wraps).
pub fn pick_named(names: &[String], current: &str, spec: &str) -> Result<String, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("pass a name, or next".into());
    }
    if names.is_empty() {
        return Err("none available".into());
    }
    if spec.eq_ignore_ascii_case("next") {
        let i = names
            .iter()
            .position(|n| n == current)
            .unwrap_or(usize::MAX);
        let next = if i == usize::MAX { 0 } else { (i + 1) % names.len() };
        return Ok(names[next].clone());
    }
    if let Some(n) = names.iter().find(|n| n.eq_ignore_ascii_case(spec)) {
        return Ok(n.clone());
    }
    let needle = spec.to_lowercase();
    let hits: Vec<&String> = names
        .iter()
        .filter(|n| n.to_lowercase().contains(&needle))
        .collect();
    match hits.len() {
        1 => return Ok(hits[0].clone()),
        n if n > 1 => {
            return Err(format!(
                "ambiguous {spec:?}: {}",
                hits.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        _ => {}
    }
    let close: Vec<&String> = names
        .iter()
        .filter(|n| edit_distance(&n.to_lowercase(), &needle) == 1)
        .collect();
    match close.len() {
        1 => Ok(close[0].clone()),
        _ => Err(format!("no match for {spec:?}. available: {}", names.join(", "))),
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

pub fn format_choices(kind: &str, names: &[String], current: &str) -> String {
    if names.is_empty() {
        return format!("no {kind} options");
    }
    let mut s = format!("{kind} (current {current}):\n");
    for n in names {
        if n == current {
            s.push_str(&format!("- {n}  [current]\n"));
        } else {
            s.push_str(&format!("- {n}\n"));
        }
    }
    s.pop();
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_rs::Uuid;

    #[test]
    fn hide_dotted_children() {
        assert!(hide_dotted("/", "/.agent"));
        assert!(hide_dotted("/notes", "/notes/.secret"));
        assert!(!hide_dotted("/", "/notes"));
        assert!(!hide_dotted("/.agent", "/.agent/providers"));
        assert!(is_hidden("/.agent"));
        assert!(is_hidden("/notes/.secret"));
        assert!(is_hidden("/.chat"));
        assert!(!is_hidden("/talk.chat"));
        assert!(!is_hidden("."));
        assert!(!is_hidden("/"));
        assert!(!is_hidden("/notes"));
    }

    #[test]
    fn summaries() {
        assert_eq!(summary("list", &json!({})), "list /");
        assert_eq!(summary("read", &json!({"path": "/a.md"})), "read /a.md");
        assert_eq!(summary("info", &json!({"path": "/a.md"})), "info /a.md");
        assert_eq!(summary("this", &json!({})), "this chat");
        assert!(is_client("info") && is_client("this"));
        assert_eq!(summary("search", &json!({"query": "journal"})), "search journal");
        assert_eq!(
            summary("search", &json!({"query": "todo", "in": "content"})),
            "search todo in contents"
        );
        assert!(is_client("search"));
        assert_eq!(summary("create", &json!({"path": "/notes/"})), "create /notes/");
        assert_eq!(
            summary("rename", &json!({"path": "/a.md", "name": "b.md"})),
            "rename /a.md → b.md"
        );
        assert_eq!(
            summary("move", &json!({"path": "/a.md", "to": "/notes/"})),
            "move /a.md → /notes/"
        );
        assert_eq!(summary("delete", &json!({"path": "/a.md"})), "delete /a.md");
        assert_eq!(summary("recent", &json!({})), "recent");
        assert_eq!(summary("pin", &json!({})), "pins");
        assert_eq!(summary("pin", &json!({"path": "/a.md"})), "pin /a.md");
        assert_eq!(summary("pin", &json!({"path": "/a.md", "remove": true})), "unpin /a.md");
        assert_eq!(summary("duplicate", &json!({"path": "/a.md"})), "duplicate /a.md");
        assert_eq!(
            summary("share", &json!({"path": "/a.md", "username": "sam"})),
            "share /a.md with sam"
        );
        assert_eq!(summary("contacts", &json!({})), "contacts");
        assert_eq!(summary("contacts", &json!({"query": "sam"})), "contacts sam");
        assert!(is_client("contacts"));
        assert_eq!(summary("account", &json!({})), "account");
        assert_eq!(summary("now", &json!({})), "now");
        assert!(is_client("now"));
        let t = chrono::DateTime::parse_from_rfc3339("2026-09-08T15:04:00-05:00").unwrap();
        assert_eq!(format_now(t), "Tuesday 2026-09-08 15:04 -0500");
        assert_eq!(summary("hangup", &json!({})), "hang up");
        assert!(is_client("hangup"));
        assert_eq!(summary("imagine", &json!({"prompt": "a red cube"})), "imagine a red cube");
        assert_eq!(
            summary("imagine", &json!({"prompt": "x", "path": "/assets/a.png"})),
            "imagine /assets/a.png"
        );
        assert_eq!(summary("look", &json!({"path": "/a.png"})), "look /a.png");
        assert_eq!(
            summary("download", &json!({"url": "https://example.com/a.png"})),
            "download https://example.com/a.png"
        );
        assert_eq!(
            summary("download", &json!({"url": "https://example.com/a.png", "path": "/a.png"})),
            "download /a.png"
        );
        assert_eq!(summary("transcribe", &json!({"path": "/a.mp3"})), "transcribe /a.mp3");
        assert_eq!(summary("caption", &json!({"path": "/a.png"})), "read caption /a.png");
        assert_eq!(summary("caption", &json!({"path": "/a.png", "text": "red"})), "caption /a.png");
        assert_eq!(summary("transcript", &json!({"path": "/a.mp3"})), "read transcript /a.mp3");
        assert_eq!(summary("record", &json!({"text": "hi"})), "record");
        assert!(
            is_client("imagine")
                && is_client("look")
                && is_client("download")
                && is_client("transcribe")
                && is_client("record")
                && is_client("caption")
                && is_client("transcript")
        );
        assert_eq!(summary("tabs", &json!({})), "tabs");
        assert_eq!(summary("tabs", &json!({"path": "."})), "show .");
        assert_eq!(summary("tabs", &json!({"action": "back"})), "back");
        assert_eq!(summary("tabs", &json!({"action": "forward"})), "forward");
        assert_eq!(
            summary("tabs", &json!({"action": "move", "path": ".", "to": "start"})),
            "move tab . → start"
        );
        assert!(is_client("tabs"));
        assert_eq!(summary("settings", &json!({"kind": "voice"})), "list voice");
        assert_eq!(summary("settings", &json!({"kind": "voice", "name": "ara"})), "voice ara");
        assert!(
            is_client("create")
                && is_client("rename")
                && is_client("move")
                && is_client("delete")
                && is_client("recent")
                && is_client("pin")
                && is_client("duplicate")
                && is_client("share")
                && is_client("account")
                && is_client("now")
                && is_client("settings")
                && is_client("info")
                && is_client("contacts")
                && is_client("this")
                && is_client("tabs")
                && is_client("hangup")
                && is_client("imagine")
                && is_client("look")
                && is_client("download")
                && is_client("transcribe")
                && is_client("record")
                && is_client("caption")
                && is_client("transcript")
        );
        assert_eq!(summary("web_search", &json!({"query": "xai"})), "web xai");
        assert_eq!(summary("code_interpreter", &json!({"code": "print(1+1)"})), "code print(1+1)");
        assert!(is_server("code_interpreter") && is_server("code_interpreter_call"));
        assert_eq!(summary("x_keyword_search", &json!({"query": "grok"})), "x grok");
        assert_eq!(server_body("web_search", &json!({"query": "xai"})), "web_search: xai");
        assert_eq!(
            server_body("code_interpreter", &json!({"code": "print(1+1)"})),
            "code_interpreter: print(1+1)"
        );
        let py = expand_md(
            "code_interpreter",
            Some(&json!({
                "code": "print(1+1)\nprint(2)",
                "outputs": [{"type": "logs", "logs": "2\n"}]
            })),
            "code_interpreter: print(1+1)",
        );
        assert!(py.contains("```python"));
        assert!(py.contains("print(2)"));
        assert!(py.contains("2"));
        let web = expand_md(
            "web_search_call",
            Some(&json!({
                "query": "xai",
                "sources": [
                    {"url": "https://x.ai", "title": "xAI"},
                    {"url": "https://docs.x.ai"}
                ]
            })),
            "web_search_call: xai",
        );
        assert!(web.contains("[xAI](https://x.ai)"));
        assert!(web.contains("- https://docs.x.ai"));
        assert!(
            expand_md("web_search", Some(&json!({"query": "xai"})), "web_search: xai").is_empty()
        );
        let listing = expand_md("list", Some(&json!({"path": "/"})), "/notes/\n/todo.md");
        assert!(listing.contains("/todo.md"));
        assert_eq!(
            persist_body(
                "code_interpreter",
                &json!({"code": "print(1)", "outputs": [{"type": "logs", "logs": "1"}]})
            ),
            "1"
        );
        assert_eq!(
            persist_body(
                "web_search_call",
                &json!({"query": "xai", "sources": [{"url": "https://x.ai"}]})
            ),
            "https://x.ai"
        );
        assert!(is_server("web_search") && is_server("x_semantic_search"));
        assert!(!is_client("web_search") && !is_server("list"));
        let tools = all_tools();
        let arr = tools.as_array().unwrap();
        assert_eq!(arr[0]["type"], "web_search");
        assert_eq!(arr[0]["enable_image_search"], true);
        assert_eq!(arr[1]["type"], "x_search");
        assert_eq!(arr[2]["type"], "code_interpreter");
        let call = call_tools();
        let carr = call.as_array().unwrap();
        assert_eq!(carr[0]["enable_image_search"], true);
        assert!(carr.iter().all(|t| t["type"] != "code_interpreter"));
        assert!(arr.iter().any(|t| t["name"] == "list"));
        assert!(
            arr.iter()
                .any(|t| t["type"] == "function" && t["name"] == "read")
        );
        assert!(arr.iter().any(|t| t["name"] == "settings"));
        assert!(arr.iter().any(|t| t["name"] == "recent"));
        assert!(arr.iter().any(|t| t["name"] == "info"));
        assert!(arr.iter().any(|t| t["name"] == "this"));
        assert!(arr.iter().any(|t| t["name"] == "tabs"));
        assert!(arr.iter().any(|t| t["name"] == "hangup"));
        assert!(arr.iter().any(|t| t["name"] == "now"));
        assert!(arr.iter().any(|t| t["name"] == "imagine"));
        assert!(arr.iter().any(|t| t["name"] == "look"));
        assert!(arr.iter().any(|t| t["name"] == "download"));
        assert!(arr.iter().any(|t| t["name"] == "transcribe"));
        assert!(arr.iter().any(|t| t["name"] == "record"));
        assert!(arr.iter().any(|t| t["name"] == "caption"));
        assert!(arr.iter().any(|t| t["name"] == "transcript"));
        assert!(arr.iter().any(|t| t["name"] == "contacts"));
    }

    #[test]
    fn rename_rejects_paths() {
        assert!(basename("todo.md").is_ok());
        assert!(basename("").is_err());
        assert!(basename("notes/todo.md").is_err());
    }

    #[test]
    fn search_paths_and_cap() {
        let hit = |parent: &str, name: &str, folder: bool| SearchResult {
            id: Uuid::new_v4(),
            filename: name.into(),
            parent_path: parent.into(),
            is_folder: folder,
            path_indices: Vec::new(),
            path_matches: Vec::new(),
            content_matches: Vec::new(),
        };
        let hits = vec![
            hit("/", "notes", true),
            hit("/notes", ".secret", false),
            hit("/journal", "2026-09.md", false),
        ];
        let out = format_search_hits(&hits);
        assert_eq!(out, "/notes/\n/journal/2026-09.md");
    }

    #[test]
    fn list_caps_with_search_hint() {
        let names: Vec<String> = (0..LIST_CAP + 3).map(|i| format!("f{i}")).collect();
        let out = format_list_names(names);
        let lines: Vec<_> = out.lines().collect();
        assert_eq!(lines.len(), LIST_CAP + 1);
        assert!(lines.last().unwrap().contains("search this folder"));
    }

    #[test]
    fn snippet_flattens_and_marks_truncation() {
        let s = flatten_snippet("hello ", "world", " there");
        assert_eq!(s, "…hello world there…");
        let s = flatten_snippet("", "start", "");
        assert_eq!(s, "start");
        let long_suf: String = (0..SNIPPET_CAP).map(|_| 'x').collect();
        let s = flatten_snippet("", "ab ", &long_suf);
        assert!(s.ends_with('…'));
        assert!(s.chars().count() <= SNIPPET_CAP);
    }

    #[test]
    fn pick_named_exact_substring_next() {
        let names = ["Built-in".into(), "AirPods".into(), "USB Mic".into()];
        assert_eq!(pick_named(&names, "Built-in", "airpods").unwrap(), "AirPods");
        assert_eq!(pick_named(&names, "Built-in", "next").unwrap(), "AirPods");
        assert_eq!(pick_named(&names, "USB Mic", "next").unwrap(), "Built-in");
        assert!(pick_named(&names, "Built-in", "zzz").is_err());
        let voices: Vec<String> = VOICES.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(pick_named(&voices, "eve", "Karina").unwrap(), "carina");
    }

    #[test]
    fn settings_voice_list_and_set() {
        let mut state =
            SettingsState { voice: "eve".into(), input: String::new(), output: String::new() };
        let listed = settings(&json!({"kind": "voice"}), &mut state).unwrap();
        assert!(listed.contains("[current]"));
        assert!(listed.contains("eve"));
        let set = settings(&json!({"kind": "voice", "name": "ara"}), &mut state).unwrap();
        assert_eq!(set, "voice is ara");
        assert_eq!(state.voice, "ara");
        settings(&json!({"kind": "voice", "name": "next"}), &mut state).unwrap();
        assert_eq!(state.voice, "eve");
        assert!(settings(&json!({}), &mut state).is_err());
    }

    #[test]
    fn tab_move_targets() {
        assert_eq!(parse_move_to("start", 2, 4).unwrap(), 0);
        assert_eq!(parse_move_to("end", 0, 4).unwrap(), 3);
        assert_eq!(parse_move_to("left", 2, 4).unwrap(), 1);
        assert_eq!(parse_move_to("right", 2, 4).unwrap(), 3);
        assert_eq!(parse_move_to("2", 0, 4).unwrap(), 1);
        assert!(parse_move_to("0", 0, 4).is_err());
        assert!(parse_move_to("9", 0, 4).is_err());
    }

    #[test]
    fn tab_list_marks_current_and_live() {
        let id = Uuid::new_v4();
        let snaps = vec![
            TabSnap {
                file_id: id,
                path: "/a.chat".into(),
                active: false,
                live: true,
                can_back: false,
                can_forward: false,
            },
            TabSnap {
                file_id: Uuid::new_v4(),
                path: "/b.md".into(),
                active: true,
                live: false,
                can_back: true,
                can_forward: false,
            },
        ];
        let s = format_tab_list(&snaps);
        assert!(s.contains("[live]"));
        assert!(s.contains("[current]"));
        assert!(s.starts_with("tabs (current /b.md)"));
    }

    #[test]
    fn tab_back_forward_ops() {
        let empty = vec![TabSnap {
            file_id: Uuid::new_v4(),
            path: "/a.md".into(),
            active: true,
            live: false,
            can_back: false,
            can_forward: false,
        }];
        assert_eq!(tab_history_op("back", &empty).unwrap_err(), "nothing to go back to");
        assert_eq!(tab_history_op("forward", &empty).unwrap_err(), "nothing to go forward to");
        assert_eq!(tab_history_op("back", &[]).unwrap_err(), "no tabs");

        let ready = vec![TabSnap {
            file_id: Uuid::new_v4(),
            path: "/a.md".into(),
            active: true,
            live: false,
            can_back: true,
            can_forward: true,
        }];
        let (msg, ops) = tab_history_op("back", &ready).unwrap();
        assert_eq!(msg, "going back");
        assert_eq!(ops, vec![TabOp::Back]);
        let (msg, ops) = tab_history_op("forward", &ready).unwrap();
        assert_eq!(msg, "going forward");
        assert_eq!(ops, vec![TabOp::Forward]);
    }
}
