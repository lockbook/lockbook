//! Every markdown note's headings and links, for the link picker, backlinks,
//! and the edits a rename, move, or share would call for. The file cache
//! holds it; a thread reads every note once, then each note again when it's
//! written.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::service::events::Event;

use crate::file_cache::{FileCache, FilesExt, Reach};
use crate::links::{LinkKind, NoteLink, NoteText, read_note};
use crate::tab::markdown_editor::fragment::Heading;

#[derive(Default)]
pub struct NoteIndex {
    notes: HashMap<Uuid, Arc<NoteText>>,
    resolved: Mutex<Resolved>,
    /// Counts changes, for callers keeping what they drew from the index.
    version: AtomicU64,
    /// Every note has been read once.
    ready: AtomicBool,
}

/// What links reach, worked out on first ask: per note until it or the tree
/// changes, and the links reaching each file until anything does.
#[derive(Default)]
struct Resolved {
    targets: HashMap<Uuid, Vec<Option<Uuid>>>,
    inbound: Option<Arc<Inbound>>,
}

/// For each file, the links reaching it: the note and the link's place in it.
pub(crate) type Inbound = HashMap<Uuid, Vec<(Uuid, usize)>>;

/// A link to a file, and the note it's in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlink {
    pub note: Uuid,
    pub link: NoteLink,
}

impl NoteIndex {
    /// The headings of note `id`, once it has been read.
    pub fn headings(&self, id: Uuid) -> Option<Arc<Vec<Heading>>> {
        self.notes.get(&id).map(|n| n.headings.clone())
    }

    /// What was read of note `id`.
    pub fn note(&self, id: Uuid) -> Option<Arc<NoteText>> {
        self.notes.get(&id).cloned()
    }

    pub fn notes(&self) -> impl Iterator<Item = (Uuid, &NoteText)> {
        self.notes.iter().map(|(id, n)| (*id, &**n))
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    /// Whether every note has been read once, so what the index says of
    /// links is whole.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    pub fn insert(&mut self, id: Uuid, note: NoteText) {
        self.notes.insert(id, Arc::new(note));
        let mut resolved = self.resolved.lock().unwrap();
        resolved.targets.remove(&id);
        resolved.inbound = None;
        drop(resolved);
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Drops what `files` no longer has as a note.
    pub fn retain_in(&mut self, files: &FileCache) {
        self.notes
            .retain(|id, _| files.get_by_id(*id).is_some_and(is_note));
        self.tree_changed();
    }

    /// Forgets what the links reached, which a change to the tree may move.
    pub fn tree_changed(&self) {
        *self.resolved.lock().unwrap() = Resolved::default();
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// The links in other notes that reach `target`, by note path then
    /// order in the note.
    pub fn backlinks(&self, files: &impl FilesExt, target: Uuid) -> Vec<Backlink> {
        let inbound = self.inbound(files);
        let Some(links) = inbound.get(&target) else { return vec![] };
        let mut paths: HashMap<Uuid, String> = HashMap::new();
        let mut out: Vec<(&String, usize, Backlink)> = vec![];
        for &(note, _) in links.iter().filter(|(note, _)| *note != target) {
            paths
                .entry(note)
                .or_insert_with(|| files.path(note).to_lowercase());
        }
        for &(note, i) in links.iter().filter(|(note, _)| *note != target) {
            if let Some(link) = self.link(note, i) {
                out.push((&paths[&note], i, Backlink { note, link: link.clone() }));
            }
        }
        out.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        out.into_iter().map(|(_, _, b)| b).collect()
    }

    /// Link `i` of `note`.
    pub(crate) fn link(&self, note: Uuid, i: usize) -> Option<&NoteLink> {
        self.notes.get(&note)?.links.get(i)
    }

    /// The links reaching each file, resolving only the notes that changed
    /// since it was last asked.
    pub(crate) fn inbound(&self, files: &impl FilesExt) -> Arc<Inbound> {
        let mut resolved = self.resolved.lock().unwrap();
        if let Some(inbound) = &resolved.inbound {
            return inbound.clone();
        }
        let mut map: Inbound = HashMap::new();
        for (&note, text) in &self.notes {
            let targets = resolved.targets.entry(note).or_insert_with(|| {
                text.links
                    .iter()
                    .map(|link| locate(files, note, link).map(|(id, _)| id))
                    .collect()
            });
            for (i, target) in targets.iter().enumerate() {
                if let Some(target) = target {
                    map.entry(*target).or_default().push((note, i));
                }
            }
        }
        let map = Arc::new(map);
        resolved.inbound = Some(map.clone());
        map
    }

    /// Fills `files`' index on a thread of its own: every note now, a note
    /// again when it's written, the notes not yet read whenever the tree
    /// changes, and every note when another process changed them. Which
    /// files are notes comes from `lb`, not `files`: sync announces a change
    /// before the workspace has rebuilt its cache, and a note new to this
    /// device arrives with no write announced at all.
    pub fn keep(files: Arc<RwLock<FileCache>>, lb: Lb, ctx: egui::Context) {
        thread::spawn(move || {
            let mut events = lb.subscribe();
            read_notes(&files, &lb, false);
            files.read().unwrap().notes.set_ready();
            ctx.request_repaint();
            loop {
                match events.blocking_recv() {
                    Ok(Event::DocumentWritten(id, _)) => {
                        let markdown = lb.get_file_by_id(id).is_ok_and(|f| is_note(&f));
                        if let Some(note) = read(&lb, id).filter(|_| markdown) {
                            files.write().unwrap().notes.insert(id, note);
                        }
                    }
                    Ok(Event::MetadataChanged(_)) => read_notes(&files, &lb, true),
                    Ok(Event::IpcChangesApplied)
                    | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        read_notes(&files, &lb, false);
                    }
                    Ok(_) => continue,
                    Err(_) => return,
                }
                ctx.request_repaint();
            }
        });
    }
}

/// The file a link in `note` reaches, and how, by the rules the editor
/// draws it with.
pub fn locate(files: &impl FilesExt, note: Uuid, link: &NoteLink) -> Option<(Uuid, Reach)> {
    match link.kind {
        LinkKind::Wiki => files.locate_wikilink(&link.dest, note),
        LinkKind::Link | LinkKind::Image => files.locate_link(&link.dest, note),
    }
}

pub fn is_note(f: &lb_rs::model::file::File) -> bool {
    f.is_document() && f.name.ends_with(".md")
}

fn read(lb: &Lb, id: Uuid) -> Option<NoteText> {
    let bytes = lb.read_document(id, false).ok()?;
    Some(read_note(&String::from_utf8_lossy(&bytes)))
}

/// Reads every note `lb` has, in parallel, or with `only_new`, those not
/// read yet.
fn read_notes(files: &RwLock<FileCache>, lb: &Lb, only_new: bool) {
    let mut ids: Vec<Uuid> = lb
        .list_metadatas()
        .unwrap_or_default()
        .into_iter()
        .chain(lb.get_pending_share_files().unwrap_or_default())
        .filter(is_note)
        .map(|f| f.id)
        .collect();
    if only_new {
        let files = files.read().unwrap();
        ids.retain(|id| !files.notes.notes.contains_key(id));
    }
    if ids.is_empty() {
        return;
    }
    let queue = Mutex::new(ids);
    let workers = thread::available_parallelism().map_or(4, |n| n.get());
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(id) = queue.lock().unwrap().pop() {
                    // each note joins as it's read, so the first are there at once
                    if let Some(note) = read(lb, id) {
                        files.write().unwrap().notes.insert(id, note);
                    }
                }
            });
        }
    });
}
