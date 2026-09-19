use lb_rs::{
    Uuid,
    model::{
        file_metadata::FileType,
        text::{
            offset_types::{Byte, Grapheme},
            operation_types::{Operation, Replace},
        },
    },
};
use rmcp::{
    model::{Tool, ToolAnnotations},
    schemars::{self, JsonSchema},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use web_time::Instant;

use super::ToolResult;
use crate::{
    file_cache::FilesExt,
    tab::{ContentState, SessionId, TabContent},
    workspace::Workspace,
};

const MAX_TEXT: usize = 256 * 1024;
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListFiles {
    /// Optional parent UUID. Omit to search/browse the whole metadata index.
    parent_id: Option<String>,
    /// Case-insensitive filename substring, not full-text search.
    query: Option<String>,
    offset: Option<usize>,
    /// Maximum 200; defaults to 50.
    limit: Option<usize>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenDocument {
    file_id: String,
    new_tab: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Session {
    session_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadDocument {
    session_id: String,
    /// UTF-8 byte offset. Defaults to zero.
    start_byte: Option<usize>,
    /// Exclusive UTF-8 byte offset. Defaults to at most 256 KiB after start.
    end_byte: Option<usize>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditDocument {
    session_id: String,
    expected_revision: String,
    /// Inclusive UTF-8 byte offset, on a grapheme boundary.
    start_byte: usize,
    /// Exclusive UTF-8 byte offset, on a grapheme boundary. Equal to start for insertion.
    end_byte: usize,
    /// Replacement text, at most 256 KiB.
    text: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Undo {
    session_id: String,
    expected_revision: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateFile {
    parent_id: String,
    name: String,
    folder: bool,
    /// Unique retry key; reuse identical arguments on retries. Valid until MCP restarts.
    request_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Navigate {
    session_id: String,
    direction: Direction,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Direction {
    Back,
    Forward,
}

fn tool<T: JsonSchema>(name: &'static str, description: &'static str, read_only: bool) -> Tool {
    let schema = serde_json::to_value(schemars::schema_for!(T))
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    Tool::new(name, description, schema).with_annotations(ToolAnnotations::from_raw(
        None,
        Some(read_only),
        Some(matches!(name, "edit_document" | "undo")),
        Some(read_only || matches!(name, "focus_tab" | "save" | "create_file")),
        Some(false),
    ))
}
pub(super) fn tools() -> Vec<Tool> {
    vec![
        tool::<Empty>(
            "get_workspace",
            "Read active window, tab/session IDs, root and selected folder, loading and save status. Does not read document bodies.",
            true,
        ),
        tool::<ListFiles>(
            "list_files",
            "Browse file metadata or search filenames. Paginated; does not search document contents.",
            true,
        ),
        tool::<OpenDocument>(
            "open_document",
            "Open or focus a file in the active Lockbook window. Returns session_id and loading status; read_document may need a retry.",
            false,
        ),
        tool::<ReadDocument>(
            "read_document",
            "Read live text, including unsaved edits, selection, and revision from an open text document. Supports bounded UTF-8 byte ranges. Retry if loading.",
            true,
        ),
        tool::<EditDocument>(
            "edit_document",
            "Replace a range in a live text document, checking expected_revision. Uses one isolated undo group. Returns new revision; saving is separate.",
            false,
        ),
        tool::<Undo>(
            "undo",
            "Undo the last editor group only if the supplied revision is still current. Inspect state before undoing; this can undo a human edit too.",
            false,
        ),
        tool::<CreateFile>(
            "create_file",
            "Create an empty document or folder. Explicit parent and retry key required. Open and edit a document to add content.",
            false,
        ),
        tool::<Session>("focus_tab", "Focus an existing tab by session_id.", false),
        tool::<Navigate>(
            "navigate",
            "Go back or forward in an explicit tab's navigation history.",
            false,
        ),
        tool::<Session>(
            "save",
            "Queue a local save for an open tab. Returns queued, not saved or synced. Inspect get_workspace for progress.",
            false,
        ),
    ]
}
fn args<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid arguments: {e}"))
}
fn uuid(value: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|_| "Invalid UUID".into())
}
fn session(ws: &Workspace, value: &str) -> Result<SessionId, String> {
    let id = SessionId::from_uuid(uuid(value)?);
    if !ws.tab_strip.iter().any(|s| s.id == id) {
        return Err("Session is not open in the active window; call get_workspace".into());
    }
    Ok(id)
}
fn revision(id: SessionId, file: Uuid, seq: usize, text: &str) -> String {
    format!("{}:{file}:{seq}:{:x}", id.as_uuid(), Sha256::digest(text.as_bytes()))
}
fn markdown(
    ws: &mut Workspace, id: SessionId,
) -> Result<&mut crate::tab::markdown_editor::Editor, String> {
    let tab = ws
        .tabs
        .get_mut(&id)
        .ok_or("Session is not loaded; retry after opening")?;
    match &mut tab.content {
        ContentState::Open(TabContent::Markdown(md)) => Ok(md),
        ContentState::Loading(_) => Err("Document is loading; retry read_document".into()),
        _ => Err("This tool supports open text documents only".into()),
    }
}
fn check_revision(actual: &str, expected: &str) -> Result<(), String> {
    if actual != expected {
        Err("Revision conflict; read_document again before editing".into())
    } else {
        Ok(())
    }
}
fn byte_range(text: &str, start: usize, end: usize) -> Result<(), String> {
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        Err("Invalid UTF-8 byte range".into())
    } else {
        Ok(())
    }
}
fn grapheme_range(
    buffer: &lb_rs::model::text::buffer::Buffer, start: usize, end: usize,
) -> Result<(Grapheme, Grapheme), String> {
    byte_range(&buffer.current.text, start, end)?;
    let indexes = &buffer.current.segs.grapheme_indexes;
    let a = indexes
        .binary_search(&Byte(start))
        .map_err(|_| "Start splits a grapheme")?;
    let b = indexes
        .binary_search(&Byte(end))
        .map_err(|_| "End splits a grapheme")?;
    Ok((Grapheme(a), Grapheme(b)))
}
fn tab_state(ws: &Workspace, id: SessionId) -> Value {
    let tab = ws.tabs.get(&id);
    let title = tab.map(|t| ws.tab_title(t));
    let loading = tab.is_none_or(|t| matches!(t.content, ContentState::Loading(_)));
    json!({"session_id": id.as_uuid(), "title": title, "file_id": tab.and_then(|t| t.id()), "loading": loading,
        "dirty": tab.is_some_and(|t| t.is_dirty(&ws.tasks)), "read_only": tab.is_some_and(|t| t.read_only),
        "failed": tab.is_some_and(|t| matches!(t.content, ContentState::Failed(_)))})
}

pub(super) fn execute(ws: &mut Workspace, name: &str, value: Value) -> ToolResult {
    match name {
        "get_workspace" => {
            let _: Empty = args(value)?;
            let root_id = ws.files.read().unwrap().root.id;
            Ok(json!({"root_id": root_id, "selected_folder_id": ws.focused_parent,
                "active_session_id": ws.current_tab.map(|s| s.as_uuid()),
                "tabs": ws.tab_strip.iter().map(|s| tab_state(ws, s.id)).collect::<Vec<_>>(),
                "saves_idle": ws.tasks.saves_idle()}))
        }
        "list_files" => {
            let a: ListFiles = args(value)?;
            let parent = a.parent_id.as_deref().map(uuid).transpose()?;
            let query = a.query.unwrap_or_default().to_lowercase();
            let offset = a.offset.unwrap_or(0);
            let limit = a.limit.unwrap_or(50).clamp(1, 200);
            let files = ws.files.read().unwrap();
            let mut matches = files
                .all_files()
                .filter(|f| {
                    parent.is_none_or(|p| f.parent == p && f.id != p)
                        && f.name.to_lowercase().contains(&query)
                })
                .skip(offset);
            let results: Vec<_> = matches.by_ref().take(limit).map(|f| json!({"id": f.id, "parent_id": f.parent, "name": f.name, "folder": f.is_folder(), "modified": f.last_modified})).collect();
            let more = matches.next().is_some();
            Ok(
                json!({"files": results, "next_offset": if more {Some(offset.saturating_add(limit))} else {None}}),
            )
        }
        "open_document" => {
            let a: OpenDocument = args(value)?;
            let id = uuid(&a.file_id)?;
            {
                let files = ws.files.read().unwrap();
                let f = files.get_by_id(id).ok_or("File not found")?;
                if f.is_folder() {
                    return Err("Cannot open a folder as a document".into());
                }
            }
            ws.open_file(id, true, a.new_tab.unwrap_or(false));
            let current = ws.current_tab.ok_or("Document did not open")?;
            Ok(tab_state(ws, current))
        }
        "read_document" => {
            let a: ReadDocument = args(value)?;
            let id = session(ws, &a.session_id)?;
            let md = markdown(ws, id)?;
            let b = &md.edit.renderer.buffer;
            let start = a.start_byte.unwrap_or(0);
            let mut end = a
                .end_byte
                .unwrap_or_else(|| start.saturating_add(MAX_TEXT).min(b.current.text.len()));
            if a.end_byte.is_none() {
                while end > start && !b.current.text.is_char_boundary(end) {
                    end -= 1;
                }
            }
            byte_range(&b.current.text, start, end)?;
            if end - start > MAX_TEXT {
                return Err("Read range exceeds 256 KiB".into());
            }
            let selection = b.current.segs.range_to_byte(b.current.selection);
            Ok(
                json!({"session_id": id.as_uuid(), "file_id": md.edit.file_id, "revision": revision(id, md.edit.file_id, b.current.seq, &b.current.text),
                "text": &b.current.text[start..end], "start_byte": start, "end_byte": end,
                "total_bytes": b.current.text.len(), "truncated": end < b.current.text.len(),
                "selection": {"start_byte": selection.0.0, "end_byte": selection.1.0}}),
            )
        }
        "edit_document" | "undo" => {
            let (id, expected, edit) = if name == "edit_document" {
                let a: EditDocument = args(value)?;
                (session(ws, &a.session_id)?, a.expected_revision.clone(), Some(a))
            } else {
                let a: Undo = args(value)?;
                (session(ws, &a.session_id)?, a.expected_revision, None)
            };
            if ws.tabs.get(&id).is_some_and(|t| t.read_only) {
                return Err("Document is read-only".into());
            }
            let md = markdown(ws, id)?;
            let b = &mut md.edit.renderer.buffer;
            check_revision(
                &revision(id, md.edit.file_id, b.current.seq, &b.current.text),
                &expected,
            )?;
            let response = if let Some(a) = edit {
                if a.text.len() > MAX_TEXT {
                    return Err("Replacement exceeds 256 KiB".into());
                }
                let range = grapheme_range(b, a.start_byte, a.end_byte)?;
                b.queue_isolated(vec![Operation::Replace(Replace { range, text: a.text })]);
                b.update()
            } else {
                b.undo()
            };
            let new_revision = revision(id, md.edit.file_id, b.current.seq, &b.current.text);
            if response.text_updated {
                md.edit.renderer.bump_text_seq();
            }
            if response.text_updated {
                ws.tabs.get_mut(&id).unwrap().last_changed = Instant::now();
                ws.out.markdown_editor_text_updated = true;
            }
            ws.out.markdown_editor_selection_updated |= response.selection_user_moved;
            Ok(
                json!({"revision": new_revision, "changed": response.text_updated, "status": "applied", "saved": false}),
            )
        }
        "create_file" => {
            let a: CreateFile = args(value)?;
            if a.request_id.is_empty() {
                return Err("request_id is required".into());
            }
            let parent = uuid(&a.parent_id)?;
            let kind = if a.folder { FileType::Folder } else { FileType::Document };
            let file = ws
                .core
                .create_file(&a.name, &parent, kind)
                .map_err(|e| format!("Create failed: {e:?}"))?;
            ws.files.write().unwrap().insert_created_file(file.clone());
            ws.out.file_created = Some(Ok(file.clone()));
            Ok(
                json!({"id": file.id, "parent_id": file.parent, "name": file.name, "folder": file.is_folder(), "status": "saved_locally"}),
            )
        }
        "focus_tab" | "save" => {
            let a: Session = args(value)?;
            let id = session(ws, &a.session_id)?;
            if name == "focus_tab" {
                ws.make_current_by_session(id);
            } else {
                let i = ws.tab_strip.iter().position(|s| s.id == id).unwrap();
                ws.save_tab(i);
            }
            Ok(
                json!({"session_id": id.as_uuid(), "status": if name == "save" { "queued" } else { "focused" }}),
            )
        }
        "navigate" => {
            let a: Navigate = args(value)?;
            let id = session(ws, &a.session_id)?;
            ws.make_current_by_session(id);
            match a.direction {
                Direction::Back => ws.back(),
                Direction::Forward => ws.forward(),
            }
            Ok(tab_state(ws, id))
        }
        _ => Err("Unknown tool".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_ranges_and_revisions() {
        assert!(byte_range("aé", 1, 2).is_err());
        assert!(byte_range("aé", 1, 3).is_ok());
        assert!(byte_range("abc", 2, 1).is_err());
        let id = SessionId::new();
        let file = Uuid::new_v4();
        let token = revision(id, file, 4, "abc");
        assert!(check_revision(&token, &revision(id, file, 3, "abc")).is_err());
        assert!(check_revision(&token, &revision(id, file, 4, "xyz")).is_err());
        assert!(check_revision(&token, &revision(id, Uuid::new_v4(), 4, "abc")).is_err());
        assert!(check_revision(&token, &token).is_ok());
    }
    #[test]
    fn edits_require_complete_graphemes() {
        let buffer = lb_rs::model::text::buffer::Buffer::from("a👩‍💻é");
        assert!(grapheme_range(&buffer, 1, 5).is_err());
        assert!(grapheme_range(&buffer, 1, 12).is_ok());
        assert!(grapheme_range(&buffer, 12, 13).is_err());
        let empty = lb_rs::model::text::buffer::Buffer::from("");
        assert!(grapheme_range(&empty, 0, 0).is_ok());
    }
    #[test]
    fn rejects_unknown_arguments() {
        assert!(args::<Empty>(json!({"command": "anything"})).is_err());
        assert!(args::<EditDocument>(json!({"session_id": "x", "text": "oops"})).is_err());
    }
}
