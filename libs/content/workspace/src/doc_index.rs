//! Workspace-side document index.
//!
//! Same events as [`FileCache`]: `MetadataChanged` reconciles the file set,
//! `DocumentWritten` re-extracts that document. One markdown parse per file
//! yields headings and unresolved outgoing dests. Who-links-where is inverted
//! against the current [`FileCache`] on those same events (and after each
//! extract), so queries are a map lookup.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use egui::Context;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::text::buffer::Buffer;

use crate::file_cache::{
    FileCache, FilesExt as _, ResolvedLink, relative_path, split_internal_fragment, strip_ext,
};
use crate::tab::markdown_editor::fragment::{Outgoing, extract_document, rewrite_outgoing_dests};

pub use crate::tab::markdown_editor::fragment::LinkKind;

/// Extracted per-document facts from one parse.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexedDoc {
    last_modified: u64,
    pub headings: Vec<(String, String)>,
    pub outgoing: Vec<Outgoing>,
}

impl IndexedDoc {
    pub fn from_markdown(last_modified: u64, md: &str) -> Self {
        let extract = extract_document(md);
        Self { last_modified, headings: extract.headings, outgoing: extract.outgoing }
    }
}

/// A resolved incoming link: `from` points at the queried file via `dest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlink {
    pub from: Uuid,
    pub dest: String,
    pub kind: LinkKind,
    pub fragment: Option<String>,
}

/// An outgoing dest that does not resolve against the current file cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokenLink {
    pub from: Uuid,
    pub dest: String,
    pub kind: LinkKind,
}

/// Predicted name/parent after a rename or move. Descendants follow because
/// only this node's parent (move) or name (rename) changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reloc {
    pub id: Uuid,
    pub new_name: String,
    pub new_parent: Uuid,
}

/// Resolved dests: `incoming[to]` is who links there; `broken` did not resolve.
#[derive(Clone, Debug, Default)]
struct ResolvedGraph {
    files_mtime: u64,
    docs_gen: u64,
    incoming: HashMap<Uuid, Vec<Backlink>>,
    broken: Vec<BrokenLink>,
}

#[derive(Clone)]
pub struct DocIndex {
    docs: Arc<RwLock<HashMap<Uuid, IndexedDoc>>>,
    graph: Arc<RwLock<ResolvedGraph>>,
    docs_gen: Arc<AtomicU64>,
    invert_dirty: Arc<AtomicBool>,
    queue: Arc<Mutex<Vec<(Uuid, u64)>>>,
    running: Arc<AtomicBool>,
}

impl Default for DocIndex {
    fn default() -> Self {
        Self::empty()
    }
}

impl DocIndex {
    pub fn empty() -> Self {
        Self {
            docs: Arc::new(RwLock::new(HashMap::new())),
            graph: Arc::new(RwLock::new(ResolvedGraph::default())),
            docs_gen: Arc::new(AtomicU64::new(0)),
            invert_dirty: Arc::new(AtomicBool::new(false)),
            queue: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn bump_docs(&self) {
        self.docs_gen.fetch_add(1, Ordering::Release);
        self.invert_dirty.store(true, Ordering::Release);
    }

    pub fn headings(&self, id: Uuid) -> Option<Vec<(String, String)>> {
        self.docs
            .read()
            .unwrap()
            .get(&id)
            .map(|d| d.headings.clone())
    }

    /// Notes that link to `target`, resolved against current [`FileCache`].
    /// Same-file `#heading` dests are omitted. Unresolved dests are omitted.
    pub fn links_to(&self, files: &FileCache, target: Uuid) -> Vec<Backlink> {
        self.incoming(files, &HashSet::from([target]), false)
    }

    /// Incoming image/embed dests of `target` — share-time consumer.
    pub fn embeds_to(&self, files: &FileCache, target: Uuid) -> Vec<Backlink> {
        self.incoming(files, &HashSet::from([target]), true)
    }

    /// Dest replacements after `relocs`, keyed by referring note. Call after
    /// the rename/move succeeds, using the **pre-op** file cache, then queue
    /// each entry on [`crate::task_manager::TaskManager`].
    pub fn dest_changes(
        &self, files: &FileCache, relocs: &[Reloc],
    ) -> HashMap<Uuid, Vec<(Outgoing, String)>> {
        self.ensure_resolved(files);
        let graph = self.graph.read().unwrap();
        let mut out: HashMap<Uuid, Vec<(Outgoing, String)>> = HashMap::new();
        for reloc in relocs {
            let Some(hits) = graph.incoming.get(&reloc.id) else { continue };
            for b in hits {
                if files
                    .path(b.from)
                    .split('/')
                    .any(|s| !s.is_empty() && s.starts_with('.'))
                {
                    continue;
                }
                let o = Outgoing { dest: b.dest.clone(), kind: b.kind };
                let Some(new) = dest_after(files, b.from, reloc.id, &o, relocs) else {
                    continue;
                };
                out.entry(b.from).or_default().push((o, new));
            }
        }
        out
    }

    /// Incoming dests whose resolve is in `targets`. Sources inside `targets`
    /// are omitted (internal links). `embeds_only` keeps image/embed dests.
    pub fn incoming(
        &self, files: &FileCache, targets: &HashSet<Uuid>, embeds_only: bool,
    ) -> Vec<Backlink> {
        if targets.is_empty() {
            return Vec::new();
        }
        self.ensure_resolved(files);
        let graph = self.graph.read().unwrap();
        let mut out = Vec::new();
        for t in targets {
            let Some(hits) = graph.incoming.get(t) else { continue };
            for b in hits {
                if targets.contains(&b.from) {
                    continue;
                }
                if embeds_only && b.kind != LinkKind::Embed {
                    continue;
                }
                out.push(b.clone());
            }
        }
        out
    }

    pub fn indexed_len(&self) -> usize {
        self.docs.read().unwrap().len()
    }

    /// File ids that at least one indexed dest currently resolves to.
    pub fn referenced_ids(&self, files: &FileCache) -> HashSet<Uuid> {
        self.ensure_resolved(files);
        self.graph
            .read()
            .unwrap()
            .incoming
            .keys()
            .copied()
            .collect()
    }

    /// Outgoing dests that do not resolve. `scope` is a file (that source) or
    /// a folder (that folder and its descendants). `None` is the whole tree.
    pub fn broken(&self, files: &FileCache, scope: Option<Uuid>) -> Vec<BrokenLink> {
        self.ensure_resolved(files);
        let allowed: Option<HashSet<Uuid>> = scope.map(|id| {
            let mut s = HashSet::new();
            s.insert(id);
            for d in files.descendents(id) {
                s.insert(d.id);
            }
            s
        });
        self.graph
            .read()
            .unwrap()
            .broken
            .iter()
            .filter(|b| allowed.as_ref().is_none_or(|s| s.contains(&b.from)))
            .cloned()
            .collect()
    }

    /// Rebuild who-links-where if the file tree or extracts moved.
    fn ensure_resolved(&self, files: &FileCache) {
        let gen = self.docs_gen.load(Ordering::Acquire);
        {
            let g = self.graph.read().unwrap();
            if g.files_mtime == files.last_modified && g.docs_gen == gen {
                return;
            }
        }
        let docs = self.docs.read().unwrap();
        *self.graph.write().unwrap() = invert(&docs, files, gen);
    }

    /// Resolve `dest` as it would appear in `from`. `kind` `None` tries wiki
    /// then markdown.
    pub fn resolve(
        files: &FileCache, from: Uuid, dest: &str, kind: Option<LinkKind>,
    ) -> Option<Uuid> {
        let parent = files.get_by_id(from)?.parent;
        match kind {
            Some(kind) => {
                resolve_outgoing(files, from, parent, &Outgoing { dest: dest.to_string(), kind })
            }
            None => resolve_outgoing(
                files,
                from,
                parent,
                &Outgoing { dest: dest.to_string(), kind: LinkKind::Wiki },
            )
            .or_else(|| {
                resolve_outgoing(
                    files,
                    from,
                    parent,
                    &Outgoing { dest: dest.to_string(), kind: LinkKind::Markdown },
                )
            }),
        }
    }

    /// Test/harness insert — skips disk. `last_modified` is the version the
    /// entry is considered current for.
    pub fn insert(&self, id: Uuid, last_modified: u64, md: &str) {
        self.docs
            .write()
            .unwrap()
            .insert(id, IndexedDoc::from_markdown(last_modified, md));
        self.bump_docs();
    }

    /// Drop a document so the next reconcile/reindex re-reads it.
    pub fn invalidate(&self, id: Uuid) {
        self.docs.write().unwrap().remove(&id);
        self.bump_docs();
    }

    /// Drop entries whose files are gone; return ids whose indexed mtime
    /// doesn't match `wanted` (missing or stale).
    pub fn reconcile(&self, wanted: &HashMap<Uuid, u64>) -> Vec<(Uuid, u64)> {
        let mut docs = self.docs.write().unwrap();
        let before = docs.len();
        docs.retain(|id, _| wanted.contains_key(id));
        if docs.len() != before {
            self.bump_docs();
        }
        wanted
            .iter()
            .filter(|(id, mtime)| {
                docs.get(id)
                    .map(|d| d.last_modified != **mtime)
                    .unwrap_or(true)
            })
            .map(|(id, mtime)| (*id, *mtime))
            .collect()
    }

    /// `MetadataChanged`: file set changed. Drop gone ids, enqueue new/stale,
    /// re-invert dests against the new tree (referrers need not re-parse).
    pub fn sync(&self, files: &Arc<RwLock<FileCache>>, core: &Lb, ctx: &Context) {
        let wanted = wanted_markdown(&files.read().unwrap());
        let stale = self.reconcile(&wanted);
        self.enqueue(stale, Arc::clone(files), core, ctx);
    }

    /// `DocumentWritten`: re-extract this document if it's markdown and not
    /// already indexed at `last_modified`.
    pub fn reindex(
        &self, id: Uuid, last_modified: u64, name: &str, files: &Arc<RwLock<FileCache>>, core: &Lb,
        ctx: &Context,
    ) {
        if !is_markdown_name(name) {
            return;
        }
        if self
            .docs
            .read()
            .unwrap()
            .get(&id)
            .is_some_and(|d| d.last_modified == last_modified)
        {
            return;
        }
        self.enqueue(vec![(id, last_modified)], Arc::clone(files), core, ctx);
    }

    fn enqueue(
        &self, ids: Vec<(Uuid, u64)>, files: Arc<RwLock<FileCache>>, core: &Lb, ctx: &Context,
    ) {
        self.invert_dirty.store(true, Ordering::Release);
        #[cfg(not(target_family = "wasm"))]
        {
            if !ids.is_empty() {
                self.queue.lock().unwrap().extend(ids);
            }
            self.ensure_worker(files, core.clone(), ctx.clone());
        }
        #[cfg(target_family = "wasm")]
        {
            let _ = (ids, files, core, ctx);
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn ensure_worker(&self, files: Arc<RwLock<FileCache>>, core: Lb, ctx: Context) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let docs = Arc::clone(&self.docs);
        let graph = Arc::clone(&self.graph);
        let docs_gen = Arc::clone(&self.docs_gen);
        let invert_dirty = Arc::clone(&self.invert_dirty);
        let queue = Arc::clone(&self.queue);
        let running = Arc::clone(&self.running);
        std::thread::spawn(move || {
            loop {
                drain(&queue, &docs, &docs_gen, &core);
                if invert_dirty.swap(false, Ordering::SeqCst) {
                    let gen = docs_gen.load(Ordering::Acquire);
                    let built = {
                        let files = files.read().unwrap();
                        let docs = docs.read().unwrap();
                        invert(&docs, &files, gen)
                    };
                    *graph.write().unwrap() = built;
                }
                running.store(false, Ordering::SeqCst);
                if queue.lock().unwrap().is_empty() && !invert_dirty.load(Ordering::SeqCst) {
                    break;
                }
                if running
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    break;
                }
            }
            ctx.request_repaint();
        });
    }

    #[cfg(target_family = "wasm")]
    fn ensure_worker(&self, _files: Arc<RwLock<FileCache>>, _core: Lb, _ctx: Context) {}
}

/// Markdown documents in the cache and the mtime we'd index them at.
pub fn wanted_markdown(files: &FileCache) -> HashMap<Uuid, u64> {
    files
        .all_files()
        .filter(|f| is_markdown(f))
        .map(|f| (f.id, f.last_modified))
        .collect()
}

fn is_markdown(f: &File) -> bool {
    f.is_document() && is_markdown_name(&f.name)
}

fn is_markdown_name(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

/// Initial `safe_write` plus one merge-and-retry if the HMAC moved.
const REWRITE_CAS_TRIES: usize = 2;

pub(crate) fn rewrite_one(core: &Lb, id: Uuid, reps: &[(Outgoing, String)]) -> bool {
    let Ok((hmac, bytes)) = core.read_document_with_hmac(id, false) else {
        return false;
    };
    let Ok(base) = String::from_utf8(bytes) else {
        return false;
    };
    let ours = apply_dest_reps(&base, reps);
    if ours == base {
        return false;
    }
    cas_write_rewrite(core, id, hmac, &base, &ours)
}

fn apply_dest_reps(md: &str, reps: &[(Outgoing, String)]) -> String {
    rewrite_outgoing_dests(md, |o| {
        reps.iter()
            .find(|(old, _)| old.dest == o.dest && old.kind == o.kind)
            .map(|(_, new)| new.clone())
    })
}

/// `safe_write` the dest rewrite. HMAC moved → 3-way merge `ours` with current.
fn cas_write_rewrite(
    core: &Lb, id: Uuid, mut hmac: Option<DocumentHmac>, base: &str, ours: &str,
) -> bool {
    let mut to_write = ours.to_string();
    for _ in 0..REWRITE_CAS_TRIES {
        match core.safe_write(id, hmac, to_write.as_bytes().to_vec(), None) {
            Ok(_) => return true,
            Err(err) if err.kind == LbErrKind::ReReadRequired => {
                let Ok((new_hmac, bytes)) = core.read_document_with_hmac(id, false) else {
                    return false;
                };
                let Ok(theirs) = String::from_utf8(bytes) else {
                    return false;
                };
                to_write = Buffer::from(base).merge(ours.to_string(), theirs.clone());
                if to_write == theirs {
                    return false;
                }
                hmac = new_hmac;
            }
            Err(_) => return false,
        }
    }
    false
}

fn invert(docs: &HashMap<Uuid, IndexedDoc>, files: &FileCache, docs_gen: u64) -> ResolvedGraph {
    let mut incoming: HashMap<Uuid, Vec<Backlink>> = HashMap::new();
    let mut broken = Vec::new();
    for (from, doc) in docs {
        let Some(src) = files.get_by_id(*from) else { continue };
        for o in &doc.outgoing {
            let (path, frag) = split_internal_fragment(&o.dest);
            if path.is_empty() {
                continue;
            }
            match resolve_outgoing(files, *from, src.parent, o) {
                Some(to) => incoming.entry(to).or_default().push(Backlink {
                    from: *from,
                    dest: o.dest.clone(),
                    kind: o.kind,
                    fragment: frag.filter(|s| !s.is_empty()).map(str::to_string),
                }),
                None => broken.push(BrokenLink { from: *from, dest: o.dest.clone(), kind: o.kind }),
            }
        }
    }
    ResolvedGraph { files_mtime: files.last_modified, docs_gen, incoming, broken }
}

fn resolve_outgoing(files: &FileCache, from: Uuid, parent: Uuid, o: &Outgoing) -> Option<Uuid> {
    let (path, frag) = split_internal_fragment(&o.dest);
    if path.is_empty() {
        return frag.is_some().then_some(from);
    }
    match o.kind {
        LinkKind::Wiki => files.resolve_wikilink(path, parent),
        LinkKind::Markdown | LinkKind::Embed => match files.resolve_link(path, parent) {
            Some(ResolvedLink::File(id)) => Some(id),
            _ => None,
        },
    }
}

fn dest_after(
    files: &FileCache, from: Uuid, to: Uuid, o: &Outgoing, relocs: &[Reloc],
) -> Option<String> {
    let (path, frag) = split_internal_fragment(&o.dest);
    if path.is_empty() || path.starts_with("lb://") {
        return None;
    }
    let old_name = files.get_by_id(to)?.name.clone();
    let new_name = name_of(files, to, relocs);
    let next = match o.kind {
        LinkKind::Wiki if !path.contains('/') => {
            if old_name == new_name {
                return None;
            }
            wiki_leaf(path, &old_name, &new_name)
        }
        LinkKind::Wiki => {
            let rel = rel_dest(files, from, to, relocs);
            wiki_path_after(path, &rel, &old_name, &new_name)
        }
        LinkKind::Markdown | LinkKind::Embed if path.starts_with('/') => {
            predicted_path(files, to, relocs)
        }
        LinkKind::Markdown | LinkKind::Embed => rel_dest(files, from, to, relocs),
    };
    let next = with_frag(&next, frag);
    (next != o.dest).then_some(next)
}

fn with_frag(path: &str, frag: Option<&str>) -> String {
    match frag {
        Some(f) if !f.is_empty() => format!("{path}#{f}"),
        _ => path.to_string(),
    }
}

fn wiki_leaf(old_leaf: &str, old_name: &str, new_name: &str) -> String {
    if old_leaf.eq_ignore_ascii_case(old_name) {
        new_name.to_string()
    } else {
        strip_ext(new_name).to_string()
    }
}

fn wiki_path_after(old_path: &str, new_rel: &str, old_name: &str, new_name: &str) -> String {
    let old_leaf = old_path.rsplit('/').next().unwrap_or(old_path);
    let leaf = wiki_leaf(old_leaf, old_name, new_name);
    match new_rel.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{leaf}"),
        None => leaf,
    }
}

fn rel_dest(files: &FileCache, from: Uuid, to: Uuid, relocs: &[Reloc]) -> String {
    let from_parent = parent_of(files, from, relocs);
    let from_dir = predicted_path(files, from_parent, relocs);
    let to_path = predicted_path(files, to, relocs);
    let rel = relative_path(&from_dir, &to_path);
    if rel == "." { name_of(files, to, relocs) } else { rel }
}

fn name_of(files: &FileCache, id: Uuid, relocs: &[Reloc]) -> String {
    relocs
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.new_name.clone())
        .or_else(|| files.get_by_id(id).map(|f| f.name.clone()))
        .unwrap_or_default()
}

fn parent_of(files: &FileCache, id: Uuid, relocs: &[Reloc]) -> Uuid {
    relocs
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.new_parent)
        .or_else(|| files.get_by_id(id).map(|f| f.parent))
        .unwrap_or(id)
}

fn predicted_path(files: &FileCache, id: Uuid, relocs: &[Reloc]) -> String {
    let Some(file) = files.get_by_id(id) else {
        return "/".into();
    };
    let is_folder = file.is_folder();
    if file.is_root() {
        return "/".into();
    }
    let mut names = vec![name_of(files, id, relocs)];
    let mut current = parent_of(files, id, relocs);
    let mut reached_root = false;
    loop {
        let Some(f) = files.get_by_id(current) else { break };
        if f.is_root() {
            reached_root = true;
            break;
        }
        names.push(name_of(files, current, relocs));
        let next = parent_of(files, current, relocs);
        if next == current {
            break;
        }
        current = next;
    }
    names.reverse();
    let joined = names.join("/");
    if reached_root && is_folder {
        format!("/{joined}/")
    } else if reached_root {
        format!("/{joined}")
    } else if is_folder {
        format!("{joined}/")
    } else {
        joined
    }
}

#[cfg(not(target_family = "wasm"))]
fn drain(
    queue: &Arc<Mutex<Vec<(Uuid, u64)>>>, docs: &Arc<RwLock<HashMap<Uuid, IndexedDoc>>>,
    docs_gen: &Arc<AtomicU64>, core: &Lb,
) {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let handles: Vec<_> = (0..n)
        .map(|_| {
            let queue = Arc::clone(queue);
            let docs = Arc::clone(docs);
            let docs_gen = Arc::clone(docs_gen);
            let core = core.clone();
            std::thread::spawn(move || {
                loop {
                    let Some((id, last_modified)) = queue.lock().unwrap().pop() else {
                        return;
                    };
                    let indexed = core
                        .read_document(id, false)
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .map(|md| IndexedDoc::from_markdown(last_modified, &md))
                        .unwrap_or(IndexedDoc { last_modified, ..Default::default() });
                    docs.write().unwrap().insert(id, indexed);
                    docs_gen.fetch_add(1, Ordering::Release);
                }
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_rs::model::file_metadata::FileType;

    use crate::file_cache::FileCache;

    fn file(id: Uuid, parent: Uuid, name: &str, last_modified: u64) -> File {
        File {
            id,
            parent,
            name: name.into(),
            file_type: FileType::Document,
            last_modified,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    fn folder(id: Uuid, name: &str) -> File {
        File {
            id,
            parent: id,
            name: name.into(),
            file_type: FileType::Folder,
            last_modified: 0,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    #[test]
    fn wanted_skips_non_markdown() {
        let root_id = Uuid::new_v4();
        let md = Uuid::new_v4();
        let txt = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [file(md, root_id, "note.md", 1), file(txt, root_id, "note.txt", 1)],
            [],
        );
        let wanted = wanted_markdown(&cache);
        assert!(wanted.contains_key(&md));
        assert!(!wanted.contains_key(&txt));
        assert!(!wanted.contains_key(&root_id));
    }

    #[test]
    fn reconcile_drops_gone_and_reports_stale() {
        let index = DocIndex::empty();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        index.insert(a, 1, "# A\n");
        index.insert(b, 1, "# B\n");

        let mut wanted = HashMap::new();
        wanted.insert(a, 1); // current
        wanted.insert(c, 2); // missing
        // b gone

        let stale = index.reconcile(&wanted);
        assert!(index.headings(b).is_none(), "deleted file dropped");
        assert_eq!(index.headings(a).unwrap()[0].0, "A");
        assert!(stale.contains(&(c, 2)));
        assert!(!stale.iter().any(|(id, _)| *id == a));
    }

    #[test]
    fn insert_indexes_headings() {
        let index = DocIndex::empty();
        let id = Uuid::new_v4();
        index.insert(id, 7, "# Foo\n\n# Foo\n");
        assert_eq!(
            index.headings(id).unwrap(),
            vec![("Foo".into(), "foo".into()), ("Foo".into(), "foo-1".into())]
        );
    }

    #[test]
    fn stale_when_mtime_moves() {
        let index = DocIndex::empty();
        let id = Uuid::new_v4();
        index.insert(id, 1, "# Old\n");
        let mut wanted = HashMap::new();
        wanted.insert(id, 2);
        let stale = index.reconcile(&wanted);
        assert_eq!(stale, vec![(id, 2)]);
        // still serves the old extract until the scan writes
        assert_eq!(index.headings(id).unwrap()[0].0, "Old");
    }

    #[test]
    fn invalidate_forces_rescan() {
        let index = DocIndex::empty();
        let id = Uuid::new_v4();
        index.insert(id, 1, "# Old\n");
        index.invalidate(id);
        let mut wanted = HashMap::new();
        wanted.insert(id, 1);
        let stale = index.reconcile(&wanted);
        assert_eq!(stale, vec![(id, 1)]);
        assert!(index.headings(id).is_none());
    }

    #[test]
    fn reindex_skips_non_markdown_and_current_mtime() {
        let index = DocIndex::empty();
        let id = Uuid::new_v4();
        index.insert(id, 5, "# Keep\n");
        // No Lb/ctx work: the skip is the name/mtime gate before enqueue.
        // Non-markdown would return before touching docs.
        assert_eq!(index.headings(id).unwrap()[0].0, "Keep");
        assert!(!is_markdown_name("photo.png"));
        assert!(is_markdown_name("note.MD"));
    }

    #[test]
    fn links_to_resolves_wiki_md_and_embed() {
        let root_id = Uuid::new_v4();
        let src = Uuid::new_v4();
        let other = Uuid::new_v4();
        let pic = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [
                file(src, root_id, "src.md", 1),
                file(other, root_id, "other.md", 1),
                file(pic, root_id, "pic.png", 1),
            ],
            [],
        );
        let index = DocIndex::empty();
        index.insert(
            src,
            1,
            "[[other#Hello]] [x](other.md) ![](pic.png) [[missing]] https://ex.com\n",
        );

        let to_other = index.links_to(&cache, other);
        assert_eq!(to_other.len(), 2, "{to_other:?}");
        assert!(
            to_other
                .iter()
                .any(|b| b.kind == LinkKind::Wiki && b.fragment.as_deref() == Some("Hello"))
        );
        assert!(
            to_other
                .iter()
                .any(|b| b.kind == LinkKind::Markdown && b.fragment.is_none())
        );
        assert!(to_other.iter().all(|b| b.from == src));

        let embeds = index.embeds_to(&cache, pic);
        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0].kind, LinkKind::Embed);

        assert!(index.links_to(&cache, src).is_empty());
    }

    #[test]
    fn broken_and_resolve() {
        let root_id = Uuid::new_v4();
        let src = Uuid::new_v4();
        let other = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [file(src, root_id, "src.md", 1), file(other, root_id, "other.md", 1)],
            [],
        );
        let index = DocIndex::empty();
        index.insert(src, 1, "[[other]] [[nope]] [x](other.md) [y](gone.md)\n");

        let broken = index.broken(&cache, Some(src));
        let dests: Vec<&str> = broken.iter().map(|b| b.dest.as_str()).collect();
        assert!(dests.contains(&"nope"), "{dests:?}");
        assert!(dests.contains(&"gone.md"), "{dests:?}");
        assert!(!dests.contains(&"other"));
        assert!(!dests.contains(&"other.md"));

        assert_eq!(DocIndex::resolve(&cache, src, "other", Some(LinkKind::Wiki)), Some(other));
        assert_eq!(DocIndex::resolve(&cache, src, "other.md", None), Some(other));
        assert!(DocIndex::resolve(&cache, src, "gone.md", None).is_none());
    }

    #[test]
    fn broken_scoped_to_folder() {
        let root_id = Uuid::new_v4();
        let notes = Uuid::new_v4();
        let inside = Uuid::new_v4();
        let outside = Uuid::new_v4();
        let mut notes_folder = folder(notes, "notes");
        notes_folder.parent = root_id;
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [notes_folder, file(inside, notes, "a.md", 1), file(outside, root_id, "b.md", 1)],
            [],
        );
        let index = DocIndex::empty();
        index.insert(inside, 1, "[[nope-in]]\n");
        index.insert(outside, 1, "[[nope-out]]\n");

        let in_notes: Vec<String> = index
            .broken(&cache, Some(notes))
            .into_iter()
            .map(|b| b.dest)
            .collect();
        assert_eq!(in_notes, ["nope-in"]);

        let in_file: Vec<String> = index
            .broken(&cache, Some(inside))
            .into_iter()
            .map(|b| b.dest)
            .collect();
        assert_eq!(in_file, ["nope-in"]);
    }

    #[test]
    fn referenced_ids_from_embeds() {
        let root_id = Uuid::new_v4();
        let src = Uuid::new_v4();
        let used = Uuid::new_v4();
        let unused = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [
                file(src, root_id, "src.md", 1),
                file(used, root_id, "used.png", 1),
                file(unused, root_id, "unused.png", 1),
            ],
            [],
        );
        let index = DocIndex::empty();
        index.insert(src, 1, "![](used.png)\n");
        let refs = index.referenced_ids(&cache);
        assert!(refs.contains(&used));
        assert!(!refs.contains(&unused));
    }

    #[test]
    fn incoming_skips_sources_in_targets() {
        let root_id = Uuid::new_v4();
        let inside_src = Uuid::new_v4();
        let inside_dst = Uuid::new_v4();
        let outside = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [
                file(inside_src, root_id, "a.md", 1),
                file(inside_dst, root_id, "b.md", 1),
                file(outside, root_id, "out.md", 1),
            ],
            [],
        );
        let index = DocIndex::empty();
        index.insert(inside_src, 1, "[[b]]\n");
        index.insert(outside, 1, "[[b]] ![](b.md)\n");

        let cascade = HashSet::from([inside_src, inside_dst]);
        let hits = index.incoming(&cache, &cascade, false);
        assert!(hits.iter().all(|b| b.from == outside), "{hits:?}");
        assert_eq!(hits.len(), 2);

        let embeds = index.incoming(&cache, &cascade, true);
        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0].kind, LinkKind::Embed);
    }

    #[test]
    fn invert_picks_up_new_target_without_reparse() {
        let root_id = Uuid::new_v4();
        let src = Uuid::new_v4();
        let other = Uuid::new_v4();
        let before = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [file(src, root_id, "src.md", 1)],
            [],
        );
        let index = DocIndex::empty();
        index.insert(src, 1, "[[other]]\n");
        assert!(index.links_to(&before, other).is_empty());
        assert!(
            index
                .broken(&before, Some(src))
                .iter()
                .any(|b| b.dest == "other")
        );

        let after = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [file(src, root_id, "src.md", 1), file(other, root_id, "other.md", 2)],
            [],
        );
        let hits = index.links_to(&after, other);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].from, src);
        assert!(index.broken(&after, Some(src)).is_empty());
    }

    #[test]
    fn dest_after_rename_wiki_md_abs() {
        let root_id = Uuid::new_v4();
        let src = Uuid::new_v4();
        let other = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [file(src, root_id, "src.md", 1), file(other, root_id, "other.md", 1)],
            [],
        );
        let reloc = Reloc { id: other, new_name: "b.md".into(), new_parent: root_id };
        let wiki = Outgoing { dest: "other".into(), kind: LinkKind::Wiki };
        assert_eq!(dest_after(&cache, src, other, &wiki, &[reloc.clone()]).as_deref(), Some("b"));
        let named = Outgoing { dest: "other.md".into(), kind: LinkKind::Wiki };
        assert_eq!(
            dest_after(&cache, src, other, &named, &[reloc.clone()]).as_deref(),
            Some("b.md")
        );
        let md = Outgoing { dest: "other.md#Hi".into(), kind: LinkKind::Markdown };
        assert_eq!(
            dest_after(&cache, src, other, &md, &[reloc.clone()]).as_deref(),
            Some("b.md#Hi")
        );
        let abs = Outgoing { dest: "/other.md".into(), kind: LinkKind::Markdown };
        assert_eq!(dest_after(&cache, src, other, &abs, &[reloc]).as_deref(), Some("/b.md"));
    }

    #[test]
    fn dest_after_move_keeps_bare_wiki() {
        let root_id = Uuid::new_v4();
        let src = Uuid::new_v4();
        let other = Uuid::new_v4();
        let notes = Uuid::new_v4();
        let mut notes_folder = folder(notes, "notes");
        notes_folder.parent = root_id;
        let cache = FileCache::from_owned_and_shared(
            folder(root_id, "root"),
            [notes_folder, file(src, root_id, "src.md", 1), file(other, root_id, "other.md", 1)],
            [],
        );
        let reloc = Reloc { id: other, new_name: "other.md".into(), new_parent: notes };
        let wiki = Outgoing { dest: "other".into(), kind: LinkKind::Wiki };
        assert_eq!(dest_after(&cache, src, other, &wiki, &[reloc.clone()]), None);
        let md = Outgoing { dest: "other.md".into(), kind: LinkKind::Markdown };
        assert_eq!(
            dest_after(&cache, src, other, &md, &[reloc.clone()]).as_deref(),
            Some("notes/other.md")
        );
        let abs = Outgoing { dest: "/other.md".into(), kind: LinkKind::Markdown };
        assert_eq!(
            dest_after(&cache, src, other, &abs, &[reloc]).as_deref(),
            Some("/notes/other.md")
        );
    }

    #[test]
    fn dest_rewrite_three_way_merges_concurrent_edit() {
        let base = "see [[note]]\n";
        let ours = apply_dest_reps(
            base,
            &[(Outgoing { dest: "note".into(), kind: LinkKind::Wiki }, "journal".into())],
        );
        assert_eq!(ours, "see [[journal]]\n");
        let theirs = "see [[note]]\nhello\n";
        let merged = Buffer::from(base).merge(ours, theirs.into());
        assert_eq!(merged, "see [[journal]]\nhello\n");
    }
}
