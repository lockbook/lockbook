use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;
use std::sync::{Arc, RwLock};

use comrak::Arena;
use comrak::nodes::NodeValue;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::access_info::UserAccessMode;
use lb_rs::model::account::Account;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::usage::bytes_to_human;
use lb_rs::service::usage::UsageEntry;
use unicode_segmentation::UnicodeSegmentation;

use crate::file_cache::{FileCache, FilesExt, link_id, web_link};
use crate::show::DocType;
use crate::tab::markdown_editor::MdRender;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    pub name: String,
    pub is_folder: bool,
    pub kind: String,
    pub summary: String,
    pub path: String,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub title: &'static str,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub when: Option<i64>,
    pub go: Option<Go>,
    pub copy: Option<String>,
    pub tone: Tone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Go {
    Open(Uuid),
    Info(Uuid),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Plain,
    Muted,
    Warn,
}

impl Row {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into(), ..Default::default() }
    }

    fn when(mut self, ms: i64) -> Self {
        self.when = Some(ms);
        self
    }

    fn go(mut self, go: Go) -> Self {
        self.go = Some(go);
        self
    }

    fn copy(mut self) -> Self {
        self.copy = Some(self.value.clone());
        self
    }

    fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl Section {
    fn new(title: &'static str) -> Self {
        Self { title, rows: vec![] }
    }

    fn push(&mut self, row: Row) {
        self.rows.push(row);
    }
}

struct Ctx<'a> {
    account: &'a Account,
    id: Uuid,
    file: File,
}

pub fn gather(
    lb: &Lb, files: &Arc<RwLock<FileCache>>, account: &Account, id: Uuid,
) -> Result<Facts, String> {
    let file = match lb.get_file_by_id(id) {
        Ok(file) => file,
        Err(err) => match files.read().unwrap().get_by_id(id).cloned() {
            Some(file) => file,
            None => return Err(format!("This file is gone: {err}")),
        },
    };
    let cx = Ctx { account, id, file };
    let bytes = match cx.file.is_document() {
        true => Some(
            lb.read_document_with_hmac(id, false)
                .map_err(|e| e.to_string()),
        ),
        false => None,
    };
    let usage = lb.get_usage_breakdown().unwrap_or_default();
    let local_changes: HashSet<Uuid> = lb
        .get_local_changes()
        .unwrap_or_default()
        .into_iter()
        .collect();
    let pinned = lb.list_pinned().unwrap_or_default().contains(&id);
    let last_synced = lb.get_last_synced().unwrap_or_default();
    let web = lb.get_file_link_url(id).ok();
    let attachments_of = match cx.file.is_folder() {
        true => lb.attachments_of(&id).ok().flatten(),
        false => None,
    };

    let doc_type = DocType::from_name(&cx.file.name);
    let content = match &bytes {
        Some(Ok((_, bytes))) => Some(contents(doc_type, &cx.file.name, bytes, files, id)),
        _ => None,
    };

    let files = files.read().unwrap();
    let mut facts = Facts {
        name: cx.file.name.clone(),
        is_folder: cx.file.is_folder(),
        kind: kind_name(&cx.file),
        path: files.path(id),
        ..Default::default()
    };

    facts
        .sections
        .push(general(&cx, &files, pinned, attachments_of));

    let mut size_line = None;
    match (&bytes, cx.file.is_folder()) {
        (_, true) => {
            let rollup = Rollup::new(&files, id, &usage);
            size_line = Some(format!(
                "{} · {}",
                count(rollup.items(), "item", "items"),
                bytes_to_human(rollup.local)
            ));
            facts.sections.extend(rollup.sections(&files));
        }
        (Some(Ok((_, bytes))), false) => {
            size_line = Some(bytes_to_human(bytes.len() as u64));
            facts.sections.extend(content);
        }
        (Some(Err(err)), false) => {
            let mut section = Section::new("Contents");
            section.push(Row::new("Contents", format!("Couldn't read: {err}")).tone(Tone::Warn));
            facts.sections.push(section);
        }
        (None, false) => {}
    }
    facts.summary = match size_line {
        Some(size) => format!("{} · {size}", facts.kind),
        None => facts.kind.clone(),
    };

    let content_len = match &bytes {
        Some(Ok((_, bytes))) => Some(bytes.len() as u64),
        _ => None,
    };
    facts
        .sections
        .push(storage(&cx, &files, &usage, &local_changes, content_len, last_synced));
    facts.sections.push(sharing(&cx, &files));
    let hmac = match &bytes {
        Some(Ok((hmac, _))) => *hmac,
        _ => None,
    };
    facts.sections.push(identifiers(&cx, &files, web, hmac));
    Ok(facts)
}

fn kind_name(file: &File) -> String {
    if file.is_root() {
        return "Home folder".into();
    }
    if file.is_folder() {
        return "Folder".into();
    }
    let ext = extension(&file.name);
    match DocType::from_name(&file.name) {
        DocType::Markdown => "Markdown note".into(),
        DocType::PlainText => "Plain text".into(),
        DocType::SVG if ext == "draw" => "Drawing".into(),
        DocType::SVG => "Drawing (SVG)".into(),
        DocType::Image => format!("{} image", ext.to_uppercase()),
        DocType::ImageUnsupported => format!("{} image (can't preview)", ext.to_uppercase()),
        DocType::Code => format!("Source code (.{ext})"),
        DocType::PDF => "PDF document".into(),
        DocType::Chat => "Chat".into(),
        DocType::Unknown if ext.is_empty() || !file.name.contains('.') => "Document".into(),
        DocType::Unknown => format!("Document (.{ext})"),
    }
}

fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, ext)) => ext.to_lowercase(),
        None => String::new(),
    }
}

fn person(cx: &Ctx, username: &str) -> String {
    if username.is_empty() {
        "Unknown".into()
    } else if username == cx.account.username {
        format!("{username} (you)")
    } else {
        username.into()
    }
}

fn mode(mode: ShareMode) -> &'static str {
    match mode {
        ShareMode::Write => "Can edit",
        ShareMode::Read => "Can view",
    }
}

fn in_own_tree(files: &FileCache, id: Uuid) -> bool {
    files.get_by_id(id).is_some() && files.tree_root(id) == files.root.id
}

fn general(cx: &Ctx, files: &FileCache, pinned: bool, attachments_of: Option<Uuid>) -> Section {
    let mut s = Section::new("General");
    let file = &cx.file;
    s.push(Row::new("Kind", kind_name(file)));

    if file.is_root() {
        s.push(Row::new("Where", "Top of your files"));
    } else if !in_own_tree(files, cx.id) {
        s.push(Row::new("Where", "Shared with you, not yet in your files").tone(Tone::Muted));
    } else if let Some(parent) = files.get_by_id(file.parent) {
        let place = match parent.is_root() {
            true => "Home".to_string(),
            false => files.path(parent.id),
        };
        s.push(Row::new("Where", place).go(Go::Info(parent.id)));
    }

    if let Some(note) = attachments_of {
        let name = files
            .get_by_id(note)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| note.to_string());
        s.push(Row::new("Attachments of", name).go(Go::Open(note)));
    }
    if file.is_document() {
        if let Some(folder) = files.attachments_folder(cx.id) {
            let n = files
                .descendents(folder)
                .iter()
                .filter(|f| f.is_document())
                .count();
            s.push(Row::new("Attachments", count(n, "file", "files")).go(Go::Info(folder)));
        }
    }

    if file.last_modified > 0 {
        s.push(
            Row::new("Modified", absolute(file.last_modified as i64))
                .when(file.last_modified as i64),
        );
    }
    s.push(Row::new("Modified by", person(cx, &file.last_modified_by)));
    s.push(Row::new("Owner", person(cx, &file.owner)));
    if files.get_by_id(cx.id).is_some() {
        let access = match files.access(cx.id, cx.account) {
            UserAccessMode::Owner if file.owner == cx.account.username => "Owner",
            UserAccessMode::Owner => "Full access",
            UserAccessMode::Write => "Can edit",
            UserAccessMode::Read => "Can view",
        };
        s.push(Row::new("Your access", access));
    }
    if file.is_document() && in_own_tree(files, cx.id) {
        s.push(Row::new("Pinned", if pinned { "Yes" } else { "No" }));
    }
    s
}

fn storage(
    cx: &Ctx, files: &FileCache, usage: &[UsageEntry], local_changes: &HashSet<Uuid>,
    content_len: Option<u64>, last_synced: i64,
) -> Section {
    let mut s = Section::new("Storage and sync");
    let mut ids: Vec<Uuid> = vec![cx.id];
    if cx.file.is_folder() {
        ids.extend(files.descendents(cx.id).iter().map(|f| f.id));
    }
    let wanted: HashSet<Uuid> = ids.iter().copied().collect();
    let (mut synced, mut local) = (0u64, 0u64);
    for entry in usage.iter().filter(|e| wanted.contains(&e.id)) {
        synced += entry.synced;
        local += entry.local;
    }

    if let Some(len) = content_len {
        s.push(Row::new("Size", exact_size(len)));
    }
    let own = cx.file.owner == cx.account.username;
    if own {
        s.push(Row::new("Stored on this device", bytes_to_human(local)));
        s.push(Row::new("Stored on server", bytes_to_human(synced)));
        if local != synced {
            let delta = if local > synced {
                format!("+{} waiting to sync", bytes_to_human(local - synced))
            } else {
                format!("−{} freed on next sync", bytes_to_human(synced - local))
            };
            s.push(Row::new("Pending", delta).tone(Tone::Warn));
        }
        let total: u64 = usage.iter().map(|e| e.local).sum();
        if total > 0 && !cx.file.is_root() {
            s.push(Row::new("Share of your storage", percent(local, total)));
        }
    } else {
        s.push(
            Row::new("Storage", format!("Counts toward {}'s storage", cx.file.owner))
                .tone(Tone::Muted),
        );
    }

    let unsynced = ids.iter().filter(|id| local_changes.contains(id)).count();
    let status = match (unsynced, cx.file.is_folder()) {
        (0, _) => Row::new("Sync status", "Up to date"),
        (_, false) => Row::new("Sync status", "Changes not yet synced").tone(Tone::Warn),
        (n, true) => Row::new(
            "Sync status",
            format!("{} with changes not yet synced", count(n, "item", "items")),
        )
        .tone(Tone::Warn),
    };
    s.push(status);
    if last_synced > 0 {
        s.push(Row::new("Last synced", absolute(last_synced)).when(last_synced));
    } else {
        s.push(Row::new("Last synced", "Never").tone(Tone::Muted));
    }
    s
}

fn sharing(cx: &Ctx, files: &FileCache) -> Section {
    let mut s = Section::new("Sharing");
    for share in &cx.file.shares {
        s.push(Row::new(
            person(cx, &share.shared_with),
            format!("{} · shared by {}", mode(share.mode), person(cx, &share.shared_by)),
        ));
    }
    for ancestor in files.ancestors(cx.id) {
        let Some(folder) = files.get_by_id(ancestor) else { continue };
        for share in &folder.shares {
            let via = match folder.is_root() {
                true => "Home".to_string(),
                false => files.path(folder.id),
            };
            s.push(
                Row::new(
                    person(cx, &share.shared_with),
                    format!("{} · through {via}", mode(share.mode)),
                )
                .go(Go::Info(folder.id)),
            );
        }
    }
    if s.rows.is_empty() {
        s.push(Row::new("Shared with", "Only you").tone(Tone::Muted));
    }
    if cx.file.is_folder() {
        let shared_inside = files
            .descendents(cx.id)
            .iter()
            .filter(|f| !f.shares.is_empty())
            .count();
        if shared_inside > 0 {
            s.push(Row::new("Shared inside", count(shared_inside, "item", "items")));
        }
    }
    s
}

fn identifiers(
    cx: &Ctx, files: &FileCache, web: Option<String>, hmac: Option<[u8; 32]>,
) -> Section {
    let mut s = Section::new("Identifiers");
    s.push(Row::new("ID", cx.id.to_string()).copy());
    if let Some(web) = web {
        s.push(Row::new("Web link", web).copy());
    }
    if !cx.file.is_root() && files.get_by_id(cx.file.parent).is_some() {
        s.push(Row::new("Parent ID", cx.file.parent.to_string()).copy());
    }
    if let Some(hmac) = hmac {
        let hex: String = hmac.iter().map(|b| format!("{b:02x}")).collect();
        let mut row = Row::new("Content version", format!("{}…", &hex[..16]));
        row.copy = Some(hex);
        s.push(row);
    } else if cx.file.is_document() {
        s.push(Row::new("Content version", "Empty, never written").tone(Tone::Muted));
    }
    s
}

pub fn contents(
    doc_type: DocType, name: &str, bytes: &[u8], files: &Arc<RwLock<FileCache>>, id: Uuid,
) -> Section {
    let mut s = Section::new("Contents");
    match doc_type {
        DocType::Markdown => match std::str::from_utf8(bytes) {
            Ok(md) => markdown(&mut s, md, files, id),
            Err(_) => s.push(Row::new("Encoding", "Not valid UTF-8").tone(Tone::Warn)),
        },
        DocType::PlainText => match std::str::from_utf8(bytes) {
            Ok(text) => plain(&mut s, text),
            Err(_) => s.push(Row::new("Encoding", "Not valid UTF-8").tone(Tone::Warn)),
        },
        DocType::Code => match std::str::from_utf8(bytes) {
            Ok(text) => code(&mut s, text),
            Err(_) => s.push(Row::new("Encoding", "Not valid UTF-8").tone(Tone::Warn)),
        },
        DocType::Image | DocType::ImageUnsupported => image(&mut s, bytes),
        DocType::PDF => pdf(&mut s, bytes),
        DocType::SVG => drawing(&mut s, bytes),
        DocType::Chat => chat(&mut s, bytes),
        DocType::Unknown => unknown(&mut s, name, bytes),
    }
    if bytes.is_empty() {
        s.rows.clear();
        s.push(Row::new("Contents", "Empty").tone(Tone::Muted));
    }
    s
}

#[derive(Debug, Default, PartialEq)]
pub struct TextStats {
    pub words: usize,
    pub chars: usize,
    pub lines: usize,
    pub blank_lines: usize,
    pub longest_line: usize,
    pub endings: &'static str,
}

pub fn text_stats(text: &str) -> TextStats {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    let endings = match (lf, crlf) {
        (0, 0) => "None (single line)",
        (_, 0) => "LF (Unix)",
        (0, _) => "CRLF (Windows)",
        _ => "Mixed LF and CRLF",
    };
    let mut lines = 0;
    let mut blank_lines = 0;
    let mut longest_line = 0;
    for line in text.lines() {
        lines += 1;
        if line.trim().is_empty() {
            blank_lines += 1;
        }
        longest_line = longest_line.max(line.chars().count());
    }
    TextStats {
        words: text.unicode_words().count(),
        chars: text.chars().filter(|c| *c != '\n' && *c != '\r').count(),
        lines,
        blank_lines,
        longest_line,
        endings,
    }
}

fn reading_time(words: usize) -> String {
    let minutes = words.div_ceil(238);
    match minutes {
        0 => "None".into(),
        1 if words < 238 => "Under a minute".into(),
        m if m < 60 => format!("About {m} min"),
        m => format!("About {} h {} min", m / 60, m % 60),
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct MarkdownStats {
    pub words: usize,
    pub chars: usize,
    pub headings: usize,
    pub top_headings: usize,
    pub tasks: usize,
    pub tasks_done: usize,
    pub web_links: usize,
    pub images: usize,
    pub code_blocks: usize,
    pub tables: usize,
    pub quotes: usize,
    pub footnotes: usize,
    pub math: usize,
    pub properties: Vec<String>,
}

pub fn markdown_stats(md: &str) -> MarkdownStats {
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, md, &MdRender::comrak_options());
    let mut stats = MarkdownStats::default();
    let mut prose = String::new();
    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Text(text) => {
                prose.push_str(text);
                prose.push(' ');
            }
            NodeValue::Code(code) => {
                prose.push_str(&code.literal);
                prose.push(' ');
            }
            NodeValue::SoftBreak | NodeValue::LineBreak => prose.push(' '),
            NodeValue::Heading(h) => {
                stats.headings += 1;
                if h.level == 1 {
                    stats.top_headings += 1;
                }
            }
            NodeValue::TaskItem(task) => {
                stats.tasks += 1;
                if task.symbol.is_some_and(|c| c != ' ') {
                    stats.tasks_done += 1;
                }
            }
            NodeValue::Link(link) if web_link(&link.url) && link_id(&link.url).is_none() => {
                stats.web_links += 1;
            }
            NodeValue::Image(_) => stats.images += 1,
            NodeValue::CodeBlock(_) => stats.code_blocks += 1,
            NodeValue::Table(_) => stats.tables += 1,
            NodeValue::BlockQuote | NodeValue::Alert(_) => stats.quotes += 1,
            NodeValue::FootnoteDefinition(_) => stats.footnotes += 1,
            NodeValue::Math(_) => stats.math += 1,
            NodeValue::FrontMatter(block) => {
                stats.properties = crate::links::properties_in(block)
                    .into_iter()
                    .map(|(key, _)| key)
                    .collect();
            }
            _ => {}
        }
    }
    stats.words = prose.unicode_words().count();
    stats.chars = prose
        .split_whitespace()
        .map(|w| w.chars().count())
        .sum::<usize>();
    stats
}

fn markdown(s: &mut Section, md: &str, files: &Arc<RwLock<FileCache>>, id: Uuid) {
    let stats = markdown_stats(md);
    let text = text_stats(md);
    s.push(Row::new("Words", thousands(stats.words)));
    s.push(Row::new("Characters", format!("{} without spaces", thousands(stats.chars))));
    s.push(Row::new("Lines", thousands(text.lines)));
    s.push(Row::new("Reading time", reading_time(stats.words)));
    if stats.headings > 0 {
        let top = match stats.top_headings {
            0 => String::new(),
            n => format!(", {n} top-level"),
        };
        s.push(Row::new("Headings", format!("{}{top}", stats.headings)));
    }
    if stats.tasks > 0 {
        s.push(Row::new("Tasks", format!("{} of {} done", stats.tasks_done, stats.tasks)));
    }

    let note = crate::links::read_note(md);
    let mut reached = 0;
    let mut broken = 0;
    let files = files.read().unwrap();
    let placed = in_own_tree(&files, id);
    for link in &note.links {
        match crate::note_index::locate(&*files, id, link) {
            Some(_) => reached += 1,
            None => broken += 1,
        }
    }
    drop(files);
    if reached + broken > 0 {
        let row = match (placed, broken) {
            (false, _) | (true, 0) => Row::new("Links to files", thousands(reached + broken)),
            (true, b) => Row::new("Links to files", format!("{reached} working, {b} broken"))
                .tone(Tone::Warn),
        };
        s.push(row);
    }
    if stats.web_links > 0 {
        s.push(Row::new("Web links", thousands(stats.web_links)));
    }
    let optional = [
        ("Images", stats.images),
        ("Code blocks", stats.code_blocks),
        ("Tables", stats.tables),
        ("Quotes and callouts", stats.quotes),
        ("Math", stats.math),
        ("Footnotes", stats.footnotes),
    ];
    for (label, n) in optional {
        if n > 0 {
            s.push(Row::new(label, thousands(n)));
        }
    }
    if !stats.properties.is_empty() {
        s.push(Row::new("Properties", stats.properties.join(", ")));
    }
    s.push(Row::new("Line endings", text.endings));
}

fn plain(s: &mut Section, text: &str) {
    let stats = text_stats(text);
    s.push(Row::new("Words", thousands(stats.words)));
    s.push(Row::new("Characters", thousands(stats.chars)));
    s.push(Row::new("Lines", thousands(stats.lines)));
    s.push(Row::new("Reading time", reading_time(stats.words)));
    s.push(Row::new("Line endings", stats.endings));
    s.push(Row::new("Encoding", "UTF-8"));
}

fn code(s: &mut Section, text: &str) {
    let stats = text_stats(text);
    s.push(Row::new("Lines", thousands(stats.lines)));
    s.push(Row::new("Lines of code", thousands(stats.lines - stats.blank_lines)));
    s.push(Row::new("Characters", thousands(stats.chars)));
    s.push(Row::new("Longest line", format!("{} characters", thousands(stats.longest_line))));
    let tabs = text.lines().filter(|l| l.starts_with('\t')).count();
    let spaces = text.lines().filter(|l| l.starts_with("  ")).count();
    let indent = match (tabs, spaces) {
        (0, 0) => None,
        (_, 0) => Some("Tabs"),
        (0, _) => Some("Spaces"),
        _ => Some("Tabs and spaces"),
    };
    if let Some(indent) = indent {
        s.push(Row::new("Indentation", indent));
    }
    s.push(Row::new("Line endings", stats.endings));
    s.push(Row::new("Encoding", "UTF-8"));
}

fn image(s: &mut Section, bytes: &[u8]) {
    use image::ImageDecoder as _;
    let reader = match image::ImageReader::new(Cursor::new(bytes)).with_guessed_format() {
        Ok(reader) => reader,
        Err(_) => {
            s.push(Row::new("Format", "Unrecognized").tone(Tone::Warn));
            return;
        }
    };
    if let Some(format) = reader.format() {
        s.push(Row::new("Format", format!("{format:?}").to_uppercase()));
    }
    match reader.into_decoder() {
        Ok(decoder) => {
            let (w, h) = decoder.dimensions();
            s.push(Row::new(
                "Dimensions",
                format!("{} × {} pixels", thousands(w as usize), thousands(h as usize)),
            ));
            if w > 0 && h > 0 {
                let mp = (w as f64 * h as f64) / 1_000_000.0;
                if mp >= 0.1 {
                    s.push(Row::new("Resolution", format!("{mp:.1} megapixels")));
                }
                s.push(Row::new("Aspect ratio", aspect(w, h)));
            }
            s.push(Row::new("Color", color_type(decoder.color_type())));
        }
        Err(err) => {
            s.push(Row::new("Dimensions", format!("Couldn't decode: {err}")).tone(Tone::Warn))
        }
    }
}

fn color_type(color: image::ColorType) -> String {
    use image::ColorType as C;
    let (model, bits) = match color {
        C::L8 => ("Grayscale", 8),
        C::La8 => ("Grayscale with alpha", 8),
        C::Rgb8 => ("RGB", 8),
        C::Rgba8 => ("RGB with alpha", 8),
        C::L16 => ("Grayscale", 16),
        C::La16 => ("Grayscale with alpha", 16),
        C::Rgb16 => ("RGB", 16),
        C::Rgba16 => ("RGB with alpha", 16),
        C::Rgb32F => ("RGB, floating point", 32),
        C::Rgba32F => ("RGB with alpha, floating point", 32),
        other => return format!("{other:?}"),
    };
    format!("{model}, {bits}-bit")
}

pub fn aspect(w: u32, h: u32) -> String {
    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    let g = gcd(w, h).max(1);
    let (rw, rh) = (w / g, h / g);
    if rw <= 32 && rh <= 32 {
        format!("{rw}:{rh}")
    } else {
        format!("{:.2}:1", w as f64 / h as f64)
    }
}

fn pdf(s: &mut Section, bytes: &[u8]) {
    let pdf = match hayro::Pdf::new(Arc::new(bytes.to_vec())) {
        Ok(pdf) => pdf,
        Err(_) => {
            s.push(Row::new("Pages", "Couldn't read this PDF").tone(Tone::Warn));
            return;
        }
    };
    let pages = pdf.pages();
    s.push(Row::new("Pages", thousands(pages.len())));
    let sizes: Vec<(f32, f32)> = pages.iter().map(|p| p.render_dimensions()).collect();
    if let Some(&(w, h)) = sizes.first() {
        let mixed = sizes
            .iter()
            .any(|&(pw, ph)| (pw - w).abs() > 1.0 || (ph - h).abs() > 1.0);
        let label = if mixed { "First page size" } else { "Page size" };
        s.push(Row::new(label, page_size(w, h)));
        if mixed {
            s.push(Row::new("Page sizes", "Mixed"));
        }
    }
    let version = format!("{:?}", pdf.version());
    if let Some(digits) = version.strip_prefix("Pdf") {
        if digits.len() == 2 {
            s.push(Row::new("PDF version", format!("{}.{}", &digits[..1], &digits[1..])));
        }
    }
}

pub fn page_size(w_pt: f32, h_pt: f32) -> String {
    let inches = |pt: f32| pt / 72.0;
    let mm = |pt: f32| pt / 72.0 * 25.4;
    let named = [
        ("Letter", 612.0, 792.0),
        ("Legal", 612.0, 1008.0),
        ("Tabloid", 792.0, 1224.0),
        ("A3", 841.9, 1190.6),
        ("A4", 595.3, 841.9),
        ("A5", 419.5, 595.3),
    ];
    let (short, long) = if w_pt < h_pt { (w_pt, h_pt) } else { (h_pt, w_pt) };
    let name = named
        .iter()
        .find(|(_, a, b)| (short - a).abs() < 3.0 && (long - b).abs() < 3.0)
        .map(|(n, _, _)| {
            let orientation = if w_pt > h_pt { ", landscape" } else { "" };
            format!(" ({n}{orientation})")
        })
        .unwrap_or_default();
    format!(
        "{:.1} × {:.1} in, {:.0} × {:.0} mm{name}",
        inches(w_pt),
        inches(h_pt),
        mm(w_pt),
        mm(h_pt)
    )
}

fn drawing(s: &mut Section, bytes: &[u8]) {
    use lb_rs::model::svg::element::EditableElement;
    let Ok(text) = std::str::from_utf8(bytes) else {
        s.push(Row::new("Encoding", "Not valid UTF-8").tone(Tone::Warn));
        return;
    };
    let buffer = lb_rs::model::svg::buffer::Buffer::new(text);
    let (mut paths, mut images, mut texts) = (0, 0, 0);
    for element in buffer.elements.values() {
        match element {
            EditableElement::Path(_) => paths += 1,
            EditableElement::Image(_) => images += 1,
            EditableElement::Text(_) => texts += 1,
        }
    }
    s.push(Row::new("Elements", thousands(buffer.elements.len())));
    s.push(Row::new("Strokes and shapes", thousands(paths)));
    if images > 0 {
        s.push(Row::new("Images", thousands(images)));
    }
    if texts > 0 {
        s.push(Row::new("Text boxes", thousands(texts)));
    }
}

fn chat(s: &mut Section, bytes: &[u8]) {
    let buffer = lb_rs::model::chat::Buffer::new(bytes);
    let messages: Vec<_> = buffer
        .messages
        .iter()
        .filter(|m| !m.content.trim().is_empty())
        .collect();
    s.push(Row::new("Messages", thousands(messages.len())));
    let mut people: BTreeMap<&str, usize> = BTreeMap::new();
    for m in &messages {
        *people.entry(m.from.as_str()).or_default() += 1;
    }
    if !people.is_empty() {
        let list: Vec<String> = people
            .iter()
            .map(|(who, n)| format!("{who} ({n})"))
            .collect();
        s.push(Row::new("Participants", list.join(", ")));
    }
    let words: usize = messages
        .iter()
        .map(|m| m.content.unicode_words().count())
        .sum();
    s.push(Row::new("Words", thousands(words)));
    if let Some(first) = messages.first().filter(|m| m.ts > 0) {
        s.push(Row::new("First message", absolute(first.ts)).when(first.ts));
    }
    if let Some(last) = messages.last().filter(|m| m.ts > 0) {
        s.push(Row::new("Last message", absolute(last.ts)).when(last.ts));
    }
}

fn unknown(s: &mut Section, name: &str, bytes: &[u8]) {
    if crate::tab::open::is_text(bytes) {
        s.push(Row::new("Contents", "Text"));
        if let Ok(text) = std::str::from_utf8(bytes) {
            let stats = text_stats(text);
            s.push(Row::new("Lines", thousands(stats.lines)));
            s.push(Row::new("Words", thousands(stats.words)));
            s.push(Row::new("Line endings", stats.endings));
        }
    } else {
        s.push(Row::new("Contents", "Binary data"));
    }
    let ext = extension(name);
    if !ext.is_empty() {
        s.push(Row::new("Extension", format!(".{ext}")));
    }
}

struct Rollup {
    folders: usize,
    docs: usize,
    direct_folders: usize,
    direct_docs: usize,
    local: u64,
    kinds: BTreeMap<String, (usize, u64)>,
    largest: Vec<(Uuid, u64)>,
    latest: Option<(Uuid, u64)>,
    deepest: usize,
}

impl Rollup {
    fn new(files: &FileCache, id: Uuid, usage: &[UsageEntry]) -> Self {
        let by_id: HashMap<Uuid, &UsageEntry> = usage.iter().map(|e| (e.id, e)).collect();
        let direct = files.children(id);
        let all = files.descendents(id);
        let base_depth = files.ancestors(id).len();
        let mut r = Rollup {
            folders: 0,
            docs: 0,
            direct_folders: direct.iter().filter(|f| f.is_folder()).count(),
            direct_docs: direct.iter().filter(|f| f.is_document()).count(),
            local: by_id.get(&id).map_or(0, |e| e.local),
            kinds: BTreeMap::new(),
            largest: vec![],
            latest: None,
            deepest: 0,
        };
        for f in all {
            let entry = by_id.get(&f.id);
            let local = entry.map_or(f.size_bytes, |e| e.local);
            r.local += local;
            r.deepest = r
                .deepest
                .max(files.ancestors(f.id).len().saturating_sub(base_depth));
            if f.is_folder() {
                r.folders += 1;
                continue;
            }
            r.docs += 1;
            let kind = r.kinds.entry(kind_name(f)).or_default();
            kind.0 += 1;
            kind.1 += local;
            r.largest.push((f.id, local));
            if r.latest.is_none_or(|(_, at)| f.last_modified > at) {
                r.latest = Some((f.id, f.last_modified));
            }
        }
        r.largest.sort_by_key(|a| std::cmp::Reverse(a.1));
        r.largest.truncate(5);
        r
    }

    fn items(&self) -> usize {
        self.direct_folders + self.direct_docs
    }

    fn sections(&self, files: &FileCache) -> Vec<Section> {
        let mut contents = Section::new("Contents");
        let direct = match (self.direct_folders, self.direct_docs) {
            (0, 0) => "Empty".to_string(),
            (0, d) => count(d, "document", "documents"),
            (f, 0) => count(f, "folder", "folders"),
            (f, d) => {
                format!("{}, {}", count(f, "folder", "folders"), count(d, "document", "documents"))
            }
        };
        contents.push(Row::new("Directly inside", direct));
        if self.folders > 0 {
            contents.push(Row::new(
                "Everything inside",
                format!(
                    "{} in {}",
                    count(self.docs, "document", "documents"),
                    count(self.folders, "folder", "folders")
                ),
            ));
            contents.push(Row::new("Nested", format!("{} levels deep", self.deepest)));
        }
        if let Some((id, at)) = self.latest.filter(|(_, at)| *at > 0) {
            let name = files
                .get_by_id(id)
                .map(|f| f.name.clone())
                .unwrap_or_default();
            let mut row = Row::new("Latest edit", format!("{name} · {}", absolute(at as i64)))
                .go(Go::Open(id));
            row.when = Some(at as i64);
            contents.push(row);
        }

        let mut sections = vec![contents];
        if !self.kinds.is_empty() {
            let mut kinds = Section::new("Kinds of files");
            let mut sorted: Vec<_> = self.kinds.iter().collect();
            sorted.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
            for (kind, (n, size)) in sorted {
                kinds.push(Row::new(
                    kind.clone(),
                    format!("{} · {}", thousands(*n), bytes_to_human(*size)),
                ));
            }
            sections.push(kinds);
        }
        if self.largest.iter().any(|(_, size)| *size > 0) {
            let mut largest = Section::new("Largest files");
            for (id, size) in &self.largest {
                let Some(f) = files.get_by_id(*id) else { continue };
                let path = files.path(f.id);
                largest.push(Row::new(bytes_to_human(*size), path).go(Go::Open(f.id)));
            }
            sections.push(largest);
        }
        sections
    }
}

pub fn count(n: usize, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{} {many}", thousands(n)),
    }
}

pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn exact_size(n: u64) -> String {
    match n {
        0..1024 => count(n as usize, "byte", "bytes"),
        _ => format!("{} ({} bytes)", bytes_to_human(n), thousands(n as usize)),
    }
}

fn percent(part: u64, whole: u64) -> String {
    let p = part as f64 / whole.max(1) as f64 * 100.0;
    if part == 0 {
        "0%".into()
    } else if p < 0.1 {
        "Less than 0.1%".into()
    } else if p < 10.0 {
        format!("{p:.1}%")
    } else {
        format!("{p:.0}%")
    }
}

pub fn absolute(ms: i64) -> String {
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(at) => at
            .with_timezone(&chrono::Local)
            .format("%b %-d, %Y at %-I:%M %p")
            .to_string(),
        None => "Unknown".into(),
    }
}

pub fn relative(ms: i64, now_ms: i64) -> String {
    use crate::show::ElapsedHumanString as _;
    let ago = now_ms - ms;
    if ago < 0 {
        return "in the future".into();
    }
    if ago < 1000 {
        return "just now".into();
    }
    time::Duration::milliseconds(ago).elapsed_human_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(count(1, "file", "files"), "1 file");
        assert_eq!(count(0, "file", "files"), "0 files");
        assert_eq!(count(2500, "file", "files"), "2,500 files");
    }

    #[test]
    fn text_stats_count_lines_and_endings() {
        let s = text_stats("one two\n\nthree\n");
        assert_eq!(s.words, 3);
        assert_eq!(s.lines, 3);
        assert_eq!(s.blank_lines, 1);
        assert_eq!(s.endings, "LF (Unix)");
        assert_eq!(text_stats("a\r\nb").endings, "CRLF (Windows)");
        assert_eq!(text_stats("a\r\nb\nc").endings, "Mixed LF and CRLF");
        assert_eq!(text_stats("solo").endings, "None (single line)");
    }

    #[test]
    fn markdown_stats_skip_markup() {
        let md = "---\ntags: [a]\nstatus: draft\n---\n# Title\n\nSome **bold** words and a [link](https://example.com).\n\n## Next\n\n- [x] done\n- [ ] todo\n\n```rust\nfn main() {}\n```\n\n![pic](pic.png)\n\n> quoted\n";
        let s = markdown_stats(md);
        assert_eq!(s.headings, 2);
        assert_eq!(s.top_headings, 1);
        assert_eq!(s.tasks, 2);
        assert_eq!(s.tasks_done, 1);
        assert_eq!(s.web_links, 1);
        assert_eq!(s.images, 1);
        assert_eq!(s.code_blocks, 1);
        assert_eq!(s.quotes, 1);
        assert_eq!(s.properties, vec!["tags".to_string(), "status".to_string()]);
        assert_eq!(s.words, 12);
    }

    #[test]
    fn aspect_ratios_reduce() {
        assert_eq!(aspect(1920, 1080), "16:9");
        assert_eq!(aspect(1000, 1000), "1:1");
        assert_eq!(aspect(1921, 1080), "1.78:1");
    }

    #[test]
    fn page_sizes_are_named() {
        assert!(page_size(612.0, 792.0).ends_with("(Letter)"));
        assert!(page_size(841.9, 595.3).ends_with("(A4, landscape)"));
        assert!(!page_size(500.0, 500.0).contains('('));
    }

    #[test]
    fn relative_times() {
        assert_eq!(relative(1000, 1500), "just now");
        assert_eq!(relative(2000, 1000), "in the future");
    }
}
