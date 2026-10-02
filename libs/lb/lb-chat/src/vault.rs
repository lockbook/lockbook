//! The librarian: search, read, list, edit, create, move, delete, and
//! request_access over the vault, every one of them behind the territory.
//! A walled path reads as nonexistent; creating or moving onto one ends the
//! run instead of answering, so a name collision cannot leak a name.

use std::time::{Duration, Instant};

use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Chat, Mention};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::path_ops::Filter;
use serde_json::{Value, json};

use crate::territory::{Territory, normalize};
use crate::tools::{ToolOutcome, Tools};
use crate::wire::{Call, ToolSchema};

const SEARCH_HITS: usize = 20;
const SNIPPET: usize = 80;
const LIST_CAP: usize = 200;
/// Above this, `read` answers with the outline unless a section was asked for.
const LONG_NOTE: usize = 24 * 1024;
const INDEX_TTL: Duration = Duration::from_secs(30);

pub struct VaultTools {
    lb: Lb,
    territory: Territory,
    index: Option<(Instant, Vec<Doc>)>,
}

struct Doc {
    path: String,
    text: String,
}

impl VaultTools {
    pub fn new(lb: Lb) -> Self {
        Self { lb, territory: Territory::default(), index: None }
    }

    pub fn territory(&self) -> &Territory {
        &self.territory
    }

    fn file_at(&self, path: &str) -> Result<Option<File>, String> {
        match self.lb.get_by_path(path) {
            Ok(file) => Ok(Some(file)),
            Err(e) if e.kind == LbErrKind::FileNonexistent => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// A file the model may touch. Walled and missing read the same.
    fn visible_file(&self, path: &str) -> Result<File, String> {
        let missing = format!("{path} does not exist");
        if !self.territory.visible(path) {
            return Err(if self.territory.walled(path) || !self.territory.allowed(path) {
                outside_or_missing(&self.territory, path)
            } else {
                missing
            });
        }
        self.file_at(path)?.ok_or(missing)
    }

    fn text_of(&self, file: &File, path: &str) -> Result<String, String> {
        if !is_text(path) {
            return Err(format!("{path} is not a text note"));
        }
        let bytes = self
            .lb
            .read_document(file.id, false)
            .map_err(|e| e.to_string())?;
        if path.ends_with(".chat") {
            return Ok(Chat::parse(&bytes).to_markdown());
        }
        String::from_utf8(bytes).map_err(|_| format!("{path} is not text"))
    }

    fn index(&mut self) -> &[Doc] {
        let fresh = self
            .index
            .as_ref()
            .is_some_and(|(built, _)| built.elapsed() < INDEX_TTL);
        if !fresh {
            let mut docs = Vec::new();
            let paths = self
                .lb
                .list_paths_with_ids(Some(Filter::DocumentsOnly))
                .unwrap_or_default();
            for (id, path) in paths {
                if !is_text(&path) || !self.territory.visible(&path) {
                    continue;
                }
                let Ok(bytes) = self.lb.read_document(id, false) else { continue };
                let text = if path.ends_with(".chat") {
                    Chat::parse(&bytes).text()
                } else {
                    String::from_utf8_lossy(&bytes).into_owned()
                };
                docs.push(Doc { path, text });
            }
            self.index = Some((Instant::now(), docs));
        }
        &self.index.as_ref().expect("built").1
    }

    fn search(&mut self, args: &Value) -> ToolOutcome {
        let query = str_arg(args, "query").trim().to_lowercase();
        let folder = str_arg(args, "folder");
        let folder = (!folder.is_empty()).then(|| normalize(&format!("{folder}/")));
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return ToolOutcome::err("search needs a query");
        }
        let mut hits: Vec<(i32, String, String)> = Vec::new();
        for doc in self.index() {
            if folder
                .as_ref()
                .is_some_and(|f| !doc.path.starts_with(f.as_str()))
            {
                continue;
            }
            let path_lower = doc.path.to_lowercase();
            let mut score = 0;
            if words.iter().all(|w| path_lower.contains(w)) {
                score += 4;
            }
            let phrase_at = find_ci(&doc.text, &query);
            if phrase_at.is_some() {
                score += 3;
            }
            let word_hits: Vec<usize> =
                words.iter().filter_map(|w| find_ci(&doc.text, w)).collect();
            if word_hits.len() == words.len() {
                score += 1;
            }
            if score == 0 && word_hits.is_empty() {
                continue;
            }
            let at = phrase_at.or_else(|| word_hits.iter().copied().min());
            let snippet = at.map(|i| snippet(&doc.text, i)).unwrap_or_default();
            hits.push((score, doc.path.clone(), snippet));
        }
        hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        if hits.is_empty() {
            return ToolOutcome::ok("no matches");
        }
        let total = hits.len();
        let mut out: Vec<String> = hits
            .into_iter()
            .take(SEARCH_HITS)
            .map(
                |(_, path, snippet)| {
                    if snippet.is_empty() { path } else { format!("{path}\n  {snippet}") }
                },
            )
            .collect();
        if total > SEARCH_HITS {
            out.push(format!("({} more; narrow the query or pass a folder)", total - SEARCH_HITS));
        }
        ToolOutcome::ok(out.join("\n"))
    }

    fn read(&mut self, args: &Value) -> ToolOutcome {
        let path = normalize(&str_arg(args, "path"));
        let file = match self.visible_file(&path) {
            Ok(f) => f,
            Err(e) => return ToolOutcome::err(e),
        };
        let text = match self.text_of(&file, &path) {
            Ok(t) => t,
            Err(e) => return ToolOutcome::err(e),
        };
        let section = str_arg(args, "section");
        if !section.is_empty() {
            return match section_of(&text, &section) {
                Some(s) => ToolOutcome::ok(s),
                None => ToolOutcome::err(format!(
                    "no heading matching {section:?}; headings:\n{}",
                    outline(&text)
                )),
            };
        }
        if text.len() > LONG_NOTE {
            return ToolOutcome::ok(format!(
                "{path} is long ({} chars). Headings:\n{}\nRead one with the section argument.",
                text.chars().count(),
                outline(&text)
            ));
        }
        ToolOutcome::ok(text)
    }

    fn list(&mut self, args: &Value) -> ToolOutcome {
        let path = str_arg(args, "path");
        let path = if path.is_empty() {
            self.territory.working_dir.clone()
        } else {
            normalize(&format!("{path}/"))
        };
        let file = match self.visible_file(&path) {
            Ok(f) => f,
            Err(e) => return ToolOutcome::err(e),
        };
        if file.file_type != FileType::Folder {
            return ToolOutcome::err(format!("{path} is a document; read it instead"));
        }
        let mut children = match self.lb.get_children(&file.id) {
            Ok(c) => c,
            Err(e) => return ToolOutcome::err(e.to_string()),
        };
        children.retain(|c| self.territory.visible(&format!("{path}{}", c.name)));
        children.sort_by(|a, b| {
            (b.file_type == FileType::Folder)
                .cmp(&(a.file_type == FileType::Folder))
                .then_with(|| a.name.cmp(&b.name))
        });
        let total = children.len();
        let mut lines: Vec<String> = children
            .iter()
            .take(LIST_CAP)
            .map(|c| {
                if c.file_type == FileType::Folder {
                    format!("{}/", c.name)
                } else {
                    c.name.clone()
                }
            })
            .collect();
        if total > LIST_CAP {
            lines.push(format!("({} more; search instead)", total - LIST_CAP));
        }
        if lines.is_empty() {
            return ToolOutcome::ok("(empty)");
        }
        ToolOutcome::ok(lines.join("\n"))
    }

    fn edit(&mut self, args: &Value) -> ToolOutcome {
        let path = normalize(&str_arg(args, "path"));
        let old = str_arg(args, "old");
        let new = str_arg(args, "new");
        let all = args.get("all").and_then(Value::as_bool).unwrap_or(false);
        if old.is_empty() {
            return ToolOutcome::err("old must not be empty; use create for a new note");
        }
        let file = match self.visible_file(&path) {
            Ok(f) => f,
            Err(e) => return ToolOutcome::err(e),
        };
        if !is_text(&path) || path.ends_with(".chat") {
            return ToolOutcome::err(format!("{path} is not an editable note"));
        }
        for _ in 0..8 {
            let (hmac, bytes) = match self.lb.read_document_with_hmac(file.id, false) {
                Ok(r) => r,
                Err(e) => return ToolOutcome::err(e.to_string()),
            };
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let count = text.matches(&old).count();
            if count == 0 {
                return ToolOutcome::err("old text not found; read the note and copy it exactly");
            }
            if count > 1 && !all {
                return ToolOutcome::err(format!(
                    "old text appears {count} times; include more context or pass all"
                ));
            }
            let edited = if all { text.replace(&old, &new) } else { text.replacen(&old, &new, 1) };
            match self.lb.safe_write(file.id, hmac, edited.into_bytes(), None) {
                Ok(_) => {
                    self.index = None;
                    return ToolOutcome::ok(format!(
                        "edited {path} ({count} replacement{})",
                        if count == 1 { "" } else { "s" }
                    ));
                }
                Err(e) if e.kind == LbErrKind::ReReadRequired => continue,
                Err(e) => return ToolOutcome::err(e.to_string()),
            }
        }
        ToolOutcome::err("the note kept changing; try again")
    }

    fn create(&mut self, args: &Value) -> ToolOutcome {
        let path = normalize(&str_arg(args, "path"));
        let text = str_arg(args, "text");
        if path.ends_with('/') {
            return ToolOutcome::err(
                "create makes notes; folders appear when a note is created inside them",
            );
        }
        if let Some(blocked) = self.target_blocked(&path) {
            return blocked;
        }
        let file = match self.lb.create_at_path(&path) {
            Ok(f) => f,
            Err(e) => return ToolOutcome::err(e.to_string()),
        };
        if let Err(e) = self.lb.write_document(file.id, text.as_bytes()) {
            return ToolOutcome::err(e.to_string());
        }
        self.index = None;
        ToolOutcome::ok(format!("created {path}"))
    }

    fn mv(&mut self, args: &Value) -> ToolOutcome {
        let path = normalize(&str_arg(args, "path"));
        let to = normalize(&str_arg(args, "to"));
        let file = match self.visible_file(&path) {
            Ok(f) => f,
            Err(e) => return ToolOutcome::err(e),
        };
        if to.ends_with('/') {
            return ToolOutcome::err("to must be the full destination path, including the name");
        }
        if let Some(blocked) = self.target_blocked(&to) {
            return blocked;
        }
        let (dir, name) = to
            .rsplit_once('/')
            .map(|(d, n)| (format!("{d}/"), n.to_string()))
            .unwrap_or(("/".into(), to.clone()));
        let parent = match self.lb.get_by_path(&dir) {
            Ok(f) => f,
            Err(_) => match self.lb.create_at_path(&dir) {
                Ok(f) => f,
                Err(e) => return ToolOutcome::err(e.to_string()),
            },
        };
        if parent.id != file.parent {
            if let Err(e) = self.lb.move_file(&file.id, &parent.id) {
                return ToolOutcome::err(e.to_string());
            }
        }
        if name != file.name {
            if let Err(e) = self.lb.rename_file(&file.id, &name) {
                return ToolOutcome::err(e.to_string());
            }
        }
        self.index = None;
        ToolOutcome::ok(format!("moved {path} to {to}"))
    }

    /// Nothing for a free, in-territory path; otherwise what stops it. A
    /// walled target aborts the run so a collision cannot reveal a name.
    fn target_blocked(&self, path: &str) -> Option<ToolOutcome> {
        if self.territory.walled(path) {
            return Some(ToolOutcome::Abort {
                text: format!("the agent tried to write to {path}, which is kept away from agents"),
            });
        }
        if !self.territory.allowed(path) {
            return Some(ToolOutcome::err(outside_or_missing(&self.territory, path)));
        }
        match self.file_at(path) {
            Ok(Some(_)) => Some(ToolOutcome::err(format!("{path} already exists"))),
            Ok(None) => None,
            Err(e) => Some(ToolOutcome::err(e)),
        }
    }

    fn delete(&mut self, args: &Value, approved: bool) -> ToolOutcome {
        let path = normalize(&str_arg(args, "path"));
        let file = match self.visible_file(&path) {
            Ok(f) => f,
            Err(e) => return ToolOutcome::err(e),
        };
        if !approved {
            return ToolOutcome::Ask { prompt: format!("Delete {path}?") };
        }
        match self.lb.delete_file(&file.id) {
            Ok(()) => {
                self.index = None;
                ToolOutcome::ok(format!("deleted {path}"))
            }
            Err(e) => ToolOutcome::err(e.to_string()),
        }
    }

    fn request_access(&mut self, args: &Value, approved: bool) -> ToolOutcome {
        let raw = str_arg(args, "path");
        let path = normalize(&raw);
        let reason = str_arg(args, "reason");
        if self.territory.walled(&path) {
            return ToolOutcome::err(format!("{path} does not exist"));
        }
        if self.territory.allowed(&path) {
            return ToolOutcome::ok(format!("{path} is already available"));
        }
        if !approved {
            let what =
                if path.ends_with('/') { "read and edit notes under" } else { "read and edit" };
            return ToolOutcome::Ask {
                prompt: format!("Let this chat {what} {path}? The agent says: {reason}"),
            };
        }
        self.territory.include.push(path.clone());
        self.index = None;
        ToolOutcome::Grant { path: path.clone(), text: format!("granted {path}") }
    }
}

impl Tools for VaultTools {
    fn schemas(&self) -> Vec<ToolSchema> {
        schemas()
    }

    fn prepare(&mut self, chat: &Chat, user: &str, working_dir: &str) {
        let settings = chat.settings_for(user);
        let territory = Territory::load(&self.lb, working_dir, &settings);
        if territory != self.territory {
            self.index = None;
        }
        self.territory = territory;
    }

    fn call(&mut self, call: &Call, approved: bool) -> ToolOutcome {
        let args = &call.args;
        match call.name.as_str() {
            "search" => self.search(args),
            "read" => self.read(args),
            "list" => self.list(args),
            "edit" => self.edit(args),
            "create" => self.create(args),
            "move" => self.mv(args),
            "delete" => self.delete(args, approved),
            "request_access" => self.request_access(args, approved),
            other => ToolOutcome::err(format!("no tool named {other}")),
        }
    }

    fn read_mention(&mut self, mention: &Mention) -> Option<String> {
        let path = normalize(&mention.path);
        let file = mention
            .id
            .and_then(|id| self.lb.get_file_by_id(id).ok())
            .or_else(|| self.visible_file(&path).ok())?;
        let path = self.lb.get_path_by_id(file.id).unwrap_or(path);
        if !self.territory.visible(&path) {
            return None;
        }
        self.text_of(&file, &path).ok()
    }
}

fn outside_or_missing(territory: &Territory, path: &str) -> String {
    if territory.walled(path) || territory.allowed(path) {
        format!("{path} does not exist")
    } else {
        format!(
            "{path} is outside this chat's folders ({}); call request_access for it",
            territory.roots().join(", ")
        )
    }
}

fn str_arg(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn is_text(path: &str) -> bool {
    path.ends_with(".md") || path.ends_with(".txt") || path.ends_with(".chat")
}

/// Byte offset of the first case-insensitive occurrence of `needle`.
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let needle: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return None;
    }
    let hay: Vec<(usize, char)> = hay
        .char_indices()
        .map(|(i, c)| (i, c.to_lowercase().next().unwrap_or(c)))
        .collect();
    (0..hay.len())
        .find(|&start| {
            needle
                .iter()
                .enumerate()
                .all(|(k, n)| hay.get(start + k).is_some_and(|(_, h)| h == n))
        })
        .map(|start| hay[start].0)
}

fn snippet(text: &str, at: usize) -> String {
    let mut start = at.saturating_sub(SNIPPET);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (at + SNIPPET).min(text.len());
    while !text.is_char_boundary(end) {
        end += 1;
    }
    let mut s = text[start..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if start > 0 {
        s.insert(0, '…');
    }
    if end < text.len() {
        s.push('…');
    }
    s
}

/// Markdown headings outside fenced code, as `(level, text)`.
fn headings(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let level = line.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&level) && line[level..].starts_with(' ') {
            out.push((level, line[level..].trim()));
        }
    }
    out
}

fn outline(text: &str) -> String {
    let lines: Vec<String> = headings(text)
        .into_iter()
        .map(|(level, h)| format!("{}{h}", "  ".repeat(level - 1)))
        .collect();
    if lines.is_empty() { "(no headings)".into() } else { lines.join("\n") }
}

/// The heading matching `section` and its body, up to the next heading of
/// the same or a higher level.
fn section_of(text: &str, section: &str) -> Option<String> {
    let wanted = section.trim().trim_start_matches('#').trim().to_lowercase();
    let mut out: Vec<&str> = Vec::new();
    let mut level = None;
    let mut fenced = false;
    for line in text.lines() {
        let fence = line.trim_start().starts_with("```");
        if fence {
            fenced = !fenced;
        }
        let this =
            if fenced || fence { 0 } else { line.bytes().take_while(|&b| b == b'#').count() };
        let is_heading = (1..=6).contains(&this) && line[this..].starts_with(' ');
        match level {
            None => {
                if is_heading && line[this..].trim().to_lowercase() == wanted {
                    level = Some(this);
                    out.push(line);
                }
            }
            Some(l) => {
                if is_heading && this <= l {
                    break;
                }
                out.push(line);
            }
        }
    }
    level.map(|_| out.join("\n"))
}

pub fn schemas() -> Vec<ToolSchema> {
    let schema = |name: &str, description: &str, props: Value, required: &[&str]| ToolSchema {
        name: name.into(),
        description: description.into(),
        parameters: json!({
            "type": "object",
            "properties": props,
            "required": required,
            "additionalProperties": false,
        }),
    };
    vec![
        schema(
            "search",
            "Find notes by words in their names or contents. Returns paths with snippets.",
            json!({
                "query": { "type": "string", "description": "words to look for" },
                "folder": { "type": "string", "description": "optional absolute folder to search within" },
            }),
            &["query"],
        ),
        schema(
            "read",
            "Read a note. Long notes answer with their headings; pass section to read one heading's content.",
            json!({
                "path": { "type": "string", "description": "absolute path of the note" },
                "section": { "type": "string", "description": "optional heading text to read" },
            }),
            &["path"],
        ),
        schema(
            "list",
            "List a folder's notes and subfolders. Defaults to the working directory.",
            json!({ "path": { "type": "string", "description": "absolute folder path" } }),
            &[],
        ),
        schema(
            "edit",
            "Replace text in a note. old must match exactly once unless all is true.",
            json!({
                "path": { "type": "string" },
                "old": { "type": "string", "description": "exact text to replace" },
                "new": { "type": "string", "description": "replacement text" },
                "all": { "type": "boolean", "description": "replace every occurrence" },
            }),
            &["path", "old", "new"],
        ),
        schema(
            "create",
            "Create a note at an absolute path ending in .md, with optional initial text.",
            json!({ "path": { "type": "string" }, "text": { "type": "string" } }),
            &["path"],
        ),
        schema(
            "move",
            "Move or rename a note or folder to a new absolute path.",
            json!({ "path": { "type": "string" }, "to": { "type": "string", "description": "full destination path" } }),
            &["path", "to"],
        ),
        schema(
            "delete",
            "Delete a note or folder. The user confirms first.",
            json!({ "path": { "type": "string" } }),
            &["path"],
        ),
        schema(
            "request_access",
            "Ask the user to let this chat read and edit a folder (path ending in /) or a note outside its folders.",
            json!({
                "path": { "type": "string" },
                "reason": { "type": "string", "description": "one sentence on why" },
            }),
            &["path", "reason"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "intro\n\n# Plan\n\nstep one\n\n## Details\n\nfine print\n\n```\n# not a heading\n```\n\n# Other\n\nend\n";

    #[test]
    fn outline_skips_fences() {
        assert_eq!(outline(NOTE), "Plan\n  Details\nOther");
    }

    #[test]
    fn section_runs_to_the_next_heading_of_equal_or_higher_level() {
        assert_eq!(
            section_of(NOTE, "plan").unwrap(),
            "# Plan\n\nstep one\n\n## Details\n\nfine print\n\n```\n# not a heading\n```\n"
        );
        assert_eq!(
            section_of(NOTE, "## Details").unwrap(),
            "## Details\n\nfine print\n\n```\n# not a heading\n```\n"
        );
        assert!(section_of(NOTE, "nope").is_none());
    }

    #[test]
    fn find_ci_and_snippet_keep_original_case() {
        let text = "Ünïcode before. The Pricing Decision was final. after";
        let at = find_ci(text, "pricing decision").unwrap();
        assert!(text[at..].starts_with("Pricing Decision"));
        assert!(snippet(text, at).contains("Pricing Decision"));
        assert_eq!(find_ci(text, "zzz"), None);
    }

    #[test]
    fn schemas_are_flat_and_closed() {
        for s in schemas() {
            assert_eq!(s.parameters["additionalProperties"], false, "{}", s.name);
            assert_eq!(s.parameters["type"], "object");
        }
    }
}
