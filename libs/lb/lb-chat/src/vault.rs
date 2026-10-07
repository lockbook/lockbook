//! The librarian: search, read, list, edit, create, move, and delete over
//! the vault, every one of them behind the territory and none of them asking.
//! A walled path reads as nonexistent; creating or moving onto one ends the
//! run instead of answering, so a name collision cannot leak a name. The
//! chat's own file reads as nonexistent too, and its path is refused.

use std::time::{Duration, Instant};

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Body, Chat, Mention};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::path_ops::Filter;
use resvg::{tiny_skia, usvg};
use serde_json::{Value, json};

use crate::context::truncate;
use crate::territory::{Territory, normalize};
use crate::tools::{ToolOutcome, Tools, pictured};
use crate::transcribe::{SIDECAR, recorded, transcribe, transcriber};
use crate::web;
use crate::wire::{Call, Media, PDF, ToolSchema};

const SEARCH_HITS: usize = 20;
const SNIPPET: usize = 80;
const LIST_CAP: usize = 200;
/// Above this, `read` answers with the outline unless a section was asked for.
const LONG_NOTE: usize = 24 * 1024;
/// The largest PDF that is sent to a model.
const PDF_MAX: usize = 8 * 1024 * 1024;
/// The size a picture is brought down to for a model: about a megapixel.
const PICTURE_PIXELS: f32 = 1024.0 * 1024.0;
/// Characters of a message's first line that name its section of a chat.
const CHAT_HEADING: usize = 60;
const INDEX_TTL: Duration = Duration::from_secs(30);

/// The note a folder keeps its standing instructions in.
pub const INSTRUCTIONS: &str = "AGENTS.md";
/// Bytes of one such note that reach the prompt.
const INSTRUCTIONS_CAP: usize = 16 * 1024;
/// What a web tool answers while the device is offline.
const OFFLINE: &str = "the device is offline: the web cannot be reached until it is back";

pub struct VaultTools {
    lb: Lb,
    /// The document holding the chat these tools serve.
    chat: Uuid,
    territory: Territory,
    index: Option<(Instant, Vec<Doc>)>,
    /// Where a web search goes, if the user has set an engine up.
    engine: Option<web::Engine>,
    /// What people said and tools returned in this chat: where an address
    /// has to have come from to be fetched.
    given: Vec<String>,
    /// The app's own word that the network is out of reach.
    offline: bool,
}

struct Doc {
    path: String,
    text: String,
}

impl VaultTools {
    pub fn new(lb: Lb, chat: Uuid) -> Self {
        let (territory, given) = (Territory::default(), Vec::new());
        Self { lb, chat, territory, index: None, engine: None, given, offline: false }
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
            return Ok(chat_text(&Chat::parse(&bytes)));
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
        if recorded(&path) {
            return match self.heard(&path) {
                Ok(said) => ToolOutcome::ok(said),
                Err(e) => ToolOutcome::err(e),
            };
        }
        if path.to_lowercase().ends_with(".pdf") {
            return match self.pdf(&path) {
                Ok(bytes) => {
                    let kb = bytes.len().div_ceil(1024);
                    ToolOutcome::ok(format!("{path} is a PDF, {kb} KB."))
                }
                Err(e) => ToolOutcome::err(e),
            };
        }
        if pictured(&path) {
            return match self.pixels(&path) {
                Ok(picture) => {
                    let (w, h) = (picture.width(), picture.height());
                    ToolOutcome::ok(format!("{path} is a picture, {w} by {h}."))
                }
                Err(e) => ToolOutcome::err(e),
            };
        }
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
                None if headings(&text).is_empty() => {
                    ToolOutcome::err(format!("{path} has no headings; read it without section"))
                }
                None => ToolOutcome::err(format!(
                    "no heading matching {section:?}; headings:\n{}",
                    outline(&text)
                )),
            };
        }
        // With no headings to read by, a long note is read from its start.
        if text.len() > LONG_NOTE && headings(&text).is_empty() {
            let rest = text.len() - LONG_NOTE;
            let start = truncate(&text, LONG_NOTE);
            return ToolOutcome::ok(format!("{start}\n({rest} more bytes not shown)"));
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

    /// What is said in the recording at `path`: from the transcript note
    /// beside it, written the first time it is asked for.
    fn heard(&mut self, path: &str) -> Result<String, String> {
        let file = self.visible_file(path)?;
        let beside = format!("{path}{SIDECAR}");
        if let Ok(Some(note)) = self.file_at(&beside) {
            return self.text_of(&note, &beside);
        }
        let (provider, model) = transcriber(&self.lb)
            .ok_or("no provider that transcribes recordings is set up; OpenAI does")?;
        let bytes = self
            .lb
            .read_document(file.id, false)
            .map_err(|e| e.to_string())?;
        let said = transcribe(&provider, model, &file.name, bytes)?;
        let note = self.lb.create_at_path(&beside).map_err(|e| e.to_string())?;
        self.lb
            .write_document(note.id, said.as_bytes())
            .map_err(|e| e.to_string())?;
        self.index = None;
        Ok(said)
    }

    /// The PDF at `path`, if it is small enough to send whole.
    fn pdf(&self, path: &str) -> Result<Vec<u8>, String> {
        let file = self.visible_file(path)?;
        let bytes = self
            .lb
            .read_document(file.id, false)
            .map_err(|e| e.to_string())?;
        if bytes.len() > PDF_MAX {
            let mb = bytes.len() / (1024 * 1024);
            return Err(format!("{path} is {mb} MB, too large to show"));
        }
        Ok(bytes)
    }

    /// The picture at `path`, decoded; a drawing is drawn on white.
    fn pixels(&self, path: &str) -> Result<DynamicImage, String> {
        let file = self.visible_file(path)?;
        let bytes = self
            .lb
            .read_document(file.id, false)
            .map_err(|e| e.to_string())?;
        let unreadable = || format!("{path} could not be read as a picture");
        if !path.to_lowercase().ends_with(".svg") {
            return image::load_from_memory(&bytes).map_err(|_| unreadable());
        }
        let tree = usvg::Tree::from_data(&bytes, &Default::default(), &Default::default())
            .map_err(|_| unreadable())?;
        let size = tree.size();
        let scale = (PICTURE_PIXELS / (size.width() * size.height()))
            .sqrt()
            .min(4.0);
        let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
        let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or_else(unreadable)?;
        pixmap.fill(tiny_skia::Color::WHITE);
        resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
        let png = pixmap.encode_png().map_err(|_| unreadable())?;
        image::load_from_memory(&png).map_err(|_| unreadable())
    }

    fn web_search(&mut self, args: &Value) -> ToolOutcome {
        if self.offline {
            return ToolOutcome::err(OFFLINE);
        }
        let Some(engine) = &self.engine else {
            return ToolOutcome::err(format!("no search engine is set up in {}", web::ENGINES));
        };
        match engine.search(&str_arg(args, "query")) {
            Ok(found) => ToolOutcome::ok(found),
            Err(e) => ToolOutcome::err(e),
        }
    }

    /// An address the model composed could carry what it has read out with
    /// it, so only one that was given to it is fetched.
    fn fetch(&mut self, args: &Value) -> ToolOutcome {
        if self.offline {
            return ToolOutcome::err(OFFLINE);
        }
        let url = str_arg(args, "url");
        let bare = url.trim_end_matches('/');
        if bare.is_empty() || !self.given.iter().any(|text| text.contains(bare)) {
            return ToolOutcome::err(format!(
                "{url} is not an address the user gave or a search, a page, or a note contained"
            ));
        }
        let start = args.get("start").and_then(Value::as_u64).unwrap_or(0);
        match web::fetch(&url, start as usize) {
            Ok(text) => ToolOutcome::ok(text),
            Err(e) => ToolOutcome::err(e),
        }
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
        if self.territory.own(path) {
            return Some(ToolOutcome::err(format!("{path} is taken; choose another path")));
        }
        match self.file_at(path) {
            Ok(Some(_)) => Some(ToolOutcome::err(format!("{path} already exists"))),
            Ok(None) => None,
            Err(e) => Some(ToolOutcome::err(e)),
        }
    }

    fn delete(&mut self, args: &Value) -> ToolOutcome {
        let path = normalize(&str_arg(args, "path"));
        let file = match self.visible_file(&path) {
            Ok(f) => f,
            Err(e) => return ToolOutcome::err(e),
        };
        match self.lb.delete_file(&file.id) {
            Ok(()) => {
                self.index = None;
                ToolOutcome::ok(format!("deleted {path}"))
            }
            Err(e) => ToolOutcome::err(e.to_string()),
        }
    }
}

impl Tools for VaultTools {
    fn schemas(&self) -> Vec<ToolSchema> {
        let web = web::schemas(self.engine.is_some());
        schemas().into_iter().chain(web).collect()
    }

    fn prepare(&mut self, chat: &Chat, user: &str, working_dir: &str) {
        let settings = chat.settings_for(user);
        let mut territory = Territory::load(&self.lb, working_dir, &settings);
        territory.own = self.lb.get_path_by_id(self.chat).ok();
        if territory != self.territory {
            self.index = None;
        }
        self.territory = territory;
        self.engine = web::Engine::load(&self.lb);
        self.offline = self.lb.status().offline;
        let given = chat.entries.iter().filter_map(|e| match &e.body {
            Body::User { text, .. } => Some(text.clone()),
            Body::Tool { result, .. } => Some(result.clone()),
            _ => None,
        });
        self.given = given.collect();
    }

    fn call(&mut self, call: &Call) -> ToolOutcome {
        let args = &call.args;
        match call.name.as_str() {
            "search" => self.search(args),
            "read" => self.read(args),
            "list" => self.list(args),
            "edit" => self.edit(args),
            "create" => self.create(args),
            "move" => self.mv(args),
            "delete" => self.delete(args),
            web::SEARCH => self.web_search(args),
            web::FETCH => self.fetch(args),
            other => ToolOutcome::err(format!("no tool named {other}")),
        }
    }

    /// Read whatever the territory is: these are the user's words to the
    /// model, not notes it found.
    fn instructions(&mut self, working_dir: &str) -> Vec<(String, String)> {
        let folders = working_dir
            .match_indices('/')
            .map(|(i, _)| &working_dir[..=i]);
        folders
            .filter_map(|folder| {
                let path = format!("{folder}{INSTRUCTIONS}");
                let file = self.lb.get_by_path(&path).ok()?;
                let bytes = self.lb.read_document(file.id, false).ok()?;
                let text = String::from_utf8(bytes).ok()?;
                (!text.trim().is_empty()).then(|| (path, truncate(text.trim(), INSTRUCTIONS_CAP)))
            })
            .collect()
    }

    fn file_of(&mut self, path: &str) -> Option<Uuid> {
        self.visible_file(&normalize(path)).ok().map(|f| f.id)
    }

    fn path_of(&mut self, file: Uuid) -> Option<String> {
        self.lb.get_path_by_id(file).ok()
    }

    /// At most about a megapixel: as JPEG, or PNG where it has an alpha.
    fn media(&mut self, path: &str) -> Option<Media> {
        let path = normalize(path);
        if path.to_lowercase().ends_with(".pdf") {
            let data = base64::encode(self.pdf(&path).ok()?);
            return Some(Media { mime: PDF.into(), data });
        }
        let mut picture = self.pixels(&path).ok()?;
        let pixels = (picture.width() * picture.height()) as f32;
        if pixels > PICTURE_PIXELS {
            let scale = (PICTURE_PIXELS / pixels).sqrt();
            let (w, h) = (picture.width() as f32 * scale, picture.height() as f32 * scale);
            picture = picture.resize(w as u32, h as u32, FilterType::Triangle);
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        let mime = if picture.color().has_alpha() {
            picture.write_to(&mut bytes, ImageFormat::Png).ok()?;
            "image/png"
        } else {
            let opaque = DynamicImage::ImageRgb8(picture.to_rgb8());
            opaque.write_to(&mut bytes, ImageFormat::Jpeg).ok()?;
            "image/jpeg"
        };
        Some(Media { mime: mime.into(), data: base64::encode(bytes.into_inner()) })
    }

    /// Beside the chat, where a pasted picture goes; a taken name gets a
    /// number.
    fn keep(&mut self, name: &str, bytes: &[u8]) -> Result<String, String> {
        let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "bin"));
        let folder = format!("{}imports/", self.territory.working_dir);
        let taken = |path: &str| self.lb.get_by_path(path).is_ok();
        let numbered = (2..).map(|n| format!("{folder}{stem}-{n}.{ext}"));
        let mut paths = std::iter::once(format!("{folder}{name}")).chain(numbered);
        let path = paths.find(|path| !taken(path)).unwrap_or_default();
        let file = self.lb.create_at_path(&path).map_err(|e| e.to_string())?;
        self.lb
            .write_document(file.id, bytes)
            .map_err(|e| e.to_string())?;
        self.index = None;
        Ok(path)
    }

    fn offline(&self) -> bool {
        self.offline
    }

    fn locate(&mut self, mention: &Mention) -> String {
        let moved = mention.id.and_then(|id| self.lb.get_path_by_id(id).ok());
        moved.unwrap_or_else(|| mention.path.clone())
    }
}

fn outside_or_missing(territory: &Territory, path: &str) -> String {
    if territory.walled(path) || territory.allowed(path) {
        format!("{path} does not exist")
    } else {
        format!("{path} is outside this chat's folders ({})", territory.roots().join(", "))
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

/// A chat as a note the tools can read by section: each message of a
/// person's is a heading, dated and named for how it starts, over the
/// replies and calls it led to.
fn chat_text(chat: &Chat) -> String {
    let mut out = String::new();
    for e in &chat.entries {
        let when = chrono::DateTime::from_timestamp_millis(e.ts)
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        match &e.body {
            Body::User { text, .. } => {
                let line = text.lines().next().unwrap_or_default();
                let opening: String = line.chars().take(CHAT_HEADING).collect();
                out.push_str(&format!("## {when} {}: {opening}\n\n{text}\n\n", e.from));
            }
            Body::Assistant { text, .. } if !text.is_empty() => {
                out.push_str(&format!("{text}\n\n"))
            }
            Body::Tool { name, args, ok, .. } => {
                let status = if *ok { "ok" } else { "failed" };
                out.push_str(&format!("- `{name}` {args} · {status}\n\n"));
            }
            _ => {}
        }
    }
    out
}

/// The headings of `text`, indented by level, each with its section's size.
fn outline(text: &str) -> String {
    let lines: Vec<String> = headings(text)
        .into_iter()
        .map(|(level, h)| {
            let size = section_of(text, h).map_or(0, |s| s.len());
            format!("{}{h} ({size} bytes)", "  ".repeat(level - 1))
        })
        .collect();
    if lines.is_empty() { "(no headings)".into() } else { lines.join("\n") }
}

/// The heading matching `section` and its body, up to the next heading of
/// the same or a higher level.
/// The section under the heading `section` names: the one that reads the
/// same, else the only one that contains it.
fn section_of(text: &str, section: &str) -> Option<String> {
    let wanted = section.trim().trim_start_matches('#').trim().to_lowercase();
    let all = headings(text);
    let named = |h: &&(usize, &str)| h.1.to_lowercase() == wanted;
    let mut holding = all.iter().filter(|h| h.1.to_lowercase().contains(&wanted));
    let wanted = match (all.iter().find(named), holding.next(), holding.next()) {
        (Some(_), ..) => wanted,
        (None, Some(only), None) if !wanted.is_empty() => only.1.to_lowercase(),
        _ => return None,
    };
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
            "Read a note. A long note answers with its headings and their sizes; pass section to read under one. A chat reads as a note with a heading per message. A picture, a drawing, or a PDF is shown to you; a recording is read as its transcript.",
            json!({
                "path": { "type": "string", "description": "absolute path of the note" },
                "section": { "type": "string", "description": "optional heading to read under; a distinct part of it is enough" },
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
            "Delete a note or folder.",
            json!({ "path": { "type": "string" } }),
            &["path"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "intro\n\n# Plan\n\nstep one\n\n## Details\n\nfine print\n\n```\n# not a heading\n```\n\n# Other\n\nend\n";

    /// A chat reads as a note with a section per message, and a section
    /// answers to any part of its heading that no other heading has.
    #[test]
    fn a_chat_has_a_section_for_each_message() {
        use lb_rs::model::chat::{Entry, Usage};
        let mut chat = Chat::default();
        chat.push(Entry::user("u", "plan the Hartford trip\nwith the dog"));
        chat.push(Entry::assistant("u", "Booked the sitter.", "m", Usage::default()));
        chat.push(Entry::user("u", "now the Seattle talk"));
        chat.push(Entry::assistant("u", "Rehearse twice.", "m", Usage::default()));
        let text = chat_text(&chat);
        let found = headings(&text);
        assert_eq!(found.len(), 2);
        assert!(found[0].1.ends_with("u: plan the Hartford trip"), "{}", found[0].1);
        let hartford = section_of(&text, "hartford").unwrap();
        assert!(hartford.contains("with the dog") && hartford.contains("Booked the sitter."));
        assert!(!hartford.contains("Seattle"));
        // What two headings share names neither.
        assert_eq!(section_of(&text, "the"), None);
    }

    #[test]
    fn outline_skips_fences() {
        let sizes: Vec<usize> = ["Plan", "Details", "Other"]
            .iter()
            .map(|h| section_of(NOTE, h).unwrap().len())
            .collect();
        assert_eq!(
            outline(NOTE),
            format!(
                "Plan ({} bytes)\n  Details ({} bytes)\nOther ({} bytes)",
                sizes[0], sizes[1], sizes[2]
            )
        );
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

    /// Nothing asks the user for anything: the folder is the only limit.
    #[test]
    fn the_tools_on_offer() {
        let names: Vec<String> = schemas().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["search", "read", "list", "edit", "create", "move", "delete"]);
    }

    #[test]
    fn schemas_are_flat_and_closed() {
        for s in schemas() {
            assert_eq!(s.parameters["additionalProperties"], false, "{}", s.name);
            assert_eq!(s.parameters["type"], "object");
        }
    }
}
