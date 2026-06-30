//! Lockbook file-tree tools for the chat agent: [`schemas`] advertises them to
//! the model and [`dispatch`] runs one by name. Provider-neutral — the loop in
//! [`super::harness`] gates each call on user approval before dispatching.

use lb_rs::Lb;
use lb_rs::blocking::Lb as BlockingLb;
use lb_rs::search::{ContentSearcher, PathSearcher};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::backend::ToolSchema;

/// Read cap, so one document can't flood the context window. Shared by
/// `read_file` and `read_pdf`.
const READ_CAP_BYTES: usize = 256 * 1024;
/// Caps on `list_paths` / `search_*`, same purpose.
const LIST_CAP: usize = 500;
const SEARCH_RESULT_CAP: usize = 10;
const SNIPPETS_PER_RESULT: usize = 3;

/// Every tool advertised to the model, as provider-neutral schemas.
pub fn schemas() -> Vec<ToolSchema> {
    let tool = |name: &str, description: &str, parameters: Value| ToolSchema {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    };
    let path_param = |desc: &str| {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": desc } },
            "required": ["path"]
        })
    };
    vec![
        tool(
            "read_file",
            "Read a UTF-8 text document from the lockbook.",
            path_param("Lockbook path, e.g. /notes/todo.md"),
        ),
        tool(
            "write_file",
            "Write a UTF-8 text document in the lockbook, replacing it if it exists. \
             Parent folders are created as needed.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Lockbook path, e.g. /notes/todo.md" },
                    "content": { "type": "string", "description": "The full document content to write" }
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            "list_dir",
            "List the entries of a lockbook folder (non-recursive). Folders are suffixed with '/'.",
            json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "Folder path; omit for the root" } },
                "required": []
            }),
        ),
        tool(
            "list_paths",
            "List every path in the lockbook (documents and folders). Prefer this over \
             walking folders with list_dir.",
            json!({ "type": "object", "properties": {}, "required": [] }),
        ),
        tool(
            "move_file",
            "Move a document or folder into another folder (same name).",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Current lockbook path" },
                    "new_parent": { "type": "string", "description": "Destination folder path" }
                },
                "required": ["path", "new_parent"]
            }),
        ),
        tool(
            "rename_file",
            "Rename a document or folder in place (same parent).",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Current lockbook path" },
                    "new_name": { "type": "string", "description": "New name, e.g. notes.md" }
                },
                "required": ["path", "new_name"]
            }),
        ),
        tool(
            "create_folder",
            "Create a folder (and any missing parents).",
            path_param("Folder path, e.g. /projects/q3"),
        ),
        tool(
            "stat",
            "Metadata for a document or folder: type, size, last modified, sharing. \
             Cheaper than reading the document.",
            path_param("Lockbook path"),
        ),
        tool(
            "read_pdf",
            "Extract the text of a PDF document in the lockbook. Works for text-based PDFs; \
             scanned/image-only PDFs yield little or nothing.",
            path_param("Lockbook path, e.g. /papers/attention.pdf"),
        ),
        tool(
            "search_content",
            "Full-text search across all markdown documents. Returns matching paths with \
             snippets, best matches first.",
            query_param(),
        ),
        tool(
            "search_paths",
            "Fuzzy search over file and folder paths (like a filename picker). Returns the \
             best-matching paths.",
            query_param(),
        ),
    ]
}

fn query_param() -> Value {
    json!({
        "type": "object",
        "properties": { "query": { "type": "string", "description": "What to search for" } },
        "required": ["query"]
    })
}

/// Run a tool by name, returning the text the model sees (errors folded in, so
/// the model can react to them rather than the turn aborting).
pub async fn dispatch(lb: &Lb, core: &BlockingLb, name: &str, args: &Value) -> String {
    run(lb, core, name, args)
        .await
        .unwrap_or_else(|e| format!("error: {e}"))
}

async fn run(lb: &Lb, core: &BlockingLb, name: &str, args: &Value) -> Result<String, String> {
    match name {
        "read_file" => read_file(lb, parse::<PathArg>(args)?.path).await,
        "write_file" => {
            let a: WriteArgs = parse(args)?;
            write_file(lb, a.path, a.content).await
        }
        "list_dir" => list_dir(lb, parse::<ListArgs>(args)?.path).await,
        "list_paths" => list_paths(lb).await,
        "move_file" => {
            let a: MoveArgs = parse(args)?;
            move_file(lb, a.path, a.new_parent).await
        }
        "rename_file" => {
            let a: RenameArgs = parse(args)?;
            rename_file(lb, a.path, a.new_name).await
        }
        "create_folder" => create_folder(lb, parse::<PathArg>(args)?.path).await,
        "stat" => stat(lb, parse::<PathArg>(args)?.path).await,
        "read_pdf" => read_pdf(lb, parse::<PathArg>(args)?.path).await,
        "search_content" => search_content(core, parse::<QueryArg>(args)?.query).await,
        "search_paths" => search_paths(lb, parse::<QueryArg>(args)?.query).await,
        other => Err(format!("unknown tool '{other}'")),
    }
}

fn parse<T: DeserializeOwned>(args: &Value) -> Result<T, String> {
    serde_json::from_value(args.clone()).map_err(|e| format!("bad arguments: {e}"))
}

#[derive(Deserialize)]
struct PathArg {
    path: String,
}
#[derive(Deserialize)]
struct WriteArgs {
    path: String,
    content: String,
}
#[derive(Deserialize)]
struct ListArgs {
    #[serde(default)]
    path: Option<String>,
}
#[derive(Deserialize)]
struct MoveArgs {
    path: String,
    new_parent: String,
}
#[derive(Deserialize)]
struct RenameArgs {
    path: String,
    new_name: String,
}
#[derive(Deserialize)]
struct QueryArg {
    query: String,
}

async fn read_file(lb: &Lb, path: String) -> Result<String, String> {
    let file = lb.get_by_path(&path).await.map_err(str_err)?;
    let bytes = lb.read_document(file.id, false).await.map_err(str_err)?;
    if bytes.len() > READ_CAP_BYTES {
        return Err(format!(
            "document is {} bytes, over the {READ_CAP_BYTES}-byte read cap",
            bytes.len()
        ));
    }
    String::from_utf8(bytes).map_err(|_| "document is not UTF-8".to_string())
}

async fn write_file(lb: &Lb, path: String, content: String) -> Result<String, String> {
    let file = match lb.get_by_path(&path).await {
        Ok(file) => file,
        Err(_) => lb.create_at_path(&path).await.map_err(str_err)?,
    };
    lb.write_document(file.id, content.as_bytes())
        .await
        .map_err(str_err)?;
    Ok(format!("wrote {} bytes to {}", content.len(), path))
}

async fn list_dir(lb: &Lb, path: Option<String>) -> Result<String, String> {
    let folder = match path.as_deref() {
        None | Some("/") | Some("") => lb.root().await.map_err(str_err)?,
        Some(path) => lb.get_by_path(path).await.map_err(str_err)?,
    };
    let children = lb.get_children(&folder.id).await.map_err(str_err)?;
    let mut entries: Vec<String> = children
        .into_iter()
        .map(|f| if f.is_folder() { format!("{}/", f.name) } else { f.name })
        .collect();
    entries.sort();
    if entries.is_empty() { Ok("(empty)".to_string()) } else { Ok(entries.join("\n")) }
}

async fn list_paths(lb: &Lb) -> Result<String, String> {
    let mut paths = lb.list_paths(None).await.map_err(str_err)?;
    paths.sort();
    let total = paths.len();
    paths.truncate(LIST_CAP);
    let mut out = paths.join("\n");
    if total > LIST_CAP {
        out.push_str(&format!("\n(+{} more)", total - LIST_CAP));
    }
    Ok(out)
}

async fn move_file(lb: &Lb, path: String, new_parent: String) -> Result<String, String> {
    let file = lb.get_by_path(&path).await.map_err(str_err)?;
    let parent = lb.get_by_path(&new_parent).await.map_err(str_err)?;
    lb.move_file(&file.id, &parent.id).await.map_err(str_err)?;
    Ok(format!("moved {path} into {new_parent}"))
}

async fn rename_file(lb: &Lb, path: String, new_name: String) -> Result<String, String> {
    let file = lb.get_by_path(&path).await.map_err(str_err)?;
    lb.rename_file(&file.id, &new_name).await.map_err(str_err)?;
    Ok(format!("renamed {path} to {new_name}"))
}

async fn create_folder(lb: &Lb, path: String) -> Result<String, String> {
    // a trailing slash is how create_at_path knows it's a folder
    let path = format!("{}/", path.trim_end_matches('/'));
    lb.create_at_path(&path).await.map_err(str_err)?;
    Ok(format!("created {path}"))
}

async fn stat(lb: &Lb, path: String) -> Result<String, String> {
    let file = lb.get_by_path(&path).await.map_err(str_err)?;
    let kind = if file.is_folder() { "folder" } else { "document" };
    let modified = chrono::DateTime::from_timestamp_millis(file.last_modified as i64)
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_default();
    let shares = if file.shares.is_empty() {
        "not shared".to_string()
    } else {
        format!("shared with {} user(s)", file.shares.len())
    };
    Ok(format!(
        "{path}: {kind}, {} bytes, modified {modified} by {}, {shares}",
        file.size_bytes, file.last_modified_by
    ))
}

async fn read_pdf(lb: &Lb, path: String) -> Result<String, String> {
    let file = lb.get_by_path(&path).await.map_err(str_err)?;
    let bytes = lb.read_document(file.id, false).await.map_err(str_err)?;
    // CPU-heavy parse off the async workers; panics in the parser (it has them)
    // surface as a join error rather than killing the driver.
    let text = tokio::task::spawn_blocking(move || pdf_extract::extract_text_from_mem(&bytes))
        .await
        .map_err(|e| format!("pdf parse crashed: {e}"))?
        .map_err(|e| format!("pdf parse failed: {e}"))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("no extractable text (scanned or image-only pdf?)".to_string());
    }
    if trimmed.len() > READ_CAP_BYTES {
        let mut end = READ_CAP_BYTES;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        return Ok(format!("{}\n(truncated)", &trimmed[..end]));
    }
    Ok(trimmed.to_string())
}

async fn search_content(core: &BlockingLb, query: String) -> Result<String, String> {
    let core = core.clone();
    // ContentSearcher reads every .md through the blocking Lb (its own worker
    // threads + blocking calls), so it can't run on a tokio worker.
    tokio::task::spawn_blocking(move || {
        let mut searcher = ContentSearcher::new(&core);
        searcher.query(&query);

        let mut out = String::new();
        for result in searcher.results().iter().take(SEARCH_RESULT_CAP) {
            out.push_str(&join_result_path(&result.parent_path, &result.filename));
            out.push('\n');
            for m in result.content_matches.iter().take(SNIPPETS_PER_RESULT) {
                if let Some((pre, hit, post)) = searcher.snippet(result.id, &m.range, 40) {
                    out.push_str(&format!("  …{pre}{hit}{post}…\n"));
                }
            }
        }
        let total = searcher.results().len();
        if total > SEARCH_RESULT_CAP {
            out.push_str(&format!("(+{} more matching documents)", total - SEARCH_RESULT_CAP));
        }
        if out.is_empty() { "(no matches)".to_string() } else { out }
    })
    .await
    .map_err(|e| format!("search task failed: {e}"))
}

async fn search_paths(lb: &Lb, query: String) -> Result<String, String> {
    let mut searcher = PathSearcher::new(lb).await;
    searcher.query(&query);
    let out = searcher
        .results()
        .iter()
        .take(20)
        .map(|r| join_result_path(&r.parent_path, &r.filename))
        .collect::<Vec<_>>()
        .join("\n");
    if out.is_empty() { Ok("(no matches)".to_string()) } else { Ok(out) }
}

/// `SearchResult.parent_path` has no trailing slash (except at the root), so
/// joining naively yields "/notesfile.md".
fn join_result_path(parent: &str, filename: &str) -> String {
    format!("{}/{filename}", parent.trim_end_matches('/'))
}

fn str_err(e: lb_rs::model::errors::LbErr) -> String {
    e.to_string()
}

/// Human one-liner for a tool call's arguments, for the approval row.
pub fn detail_for(tool_name: &str, args: &Value) -> String {
    let field = |name: &str| args.get(name).and_then(|v| v.as_str()).map(str::to_string);
    match tool_name {
        "read_file" | "read_pdf" | "stat" | "create_folder" => field("path").unwrap_or_default(),
        "write_file" => {
            let path = field("path").unwrap_or_default();
            let bytes = field("content").map(|c| c.len()).unwrap_or(0);
            format!("{path} ({bytes} bytes)")
        }
        "list_dir" => field("path").unwrap_or_else(|| "/".to_string()),
        "list_paths" => "/".to_string(),
        "move_file" => {
            format!(
                "{} → {}",
                field("path").unwrap_or_default(),
                field("new_parent").unwrap_or_default()
            )
        }
        "rename_file" => {
            format!(
                "{} → {}",
                field("path").unwrap_or_default(),
                field("new_name").unwrap_or_default()
            )
        }
        "search_content" | "search_paths" => field("query").unwrap_or_default(),
        _ => args.to_string().chars().take(80).collect(),
    }
}
