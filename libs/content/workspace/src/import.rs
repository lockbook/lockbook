//! Folder and file import: a dry-run plan, then a job that writes only what
//! differs from what's already under the destination.
//!
//! Matching is by path under the destination, so importing the same folder
//! again updates it in place instead of making `Vault-1`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::api::{FREE_TIER_USAGE_SIZE, METADATA_FEE};
use lb_rs::model::compression_service;
use lb_rs::model::crypto::AESKey;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::symkey;
use lb_rs::subscribers::status::Status;

/// Names that belong to the OS or another app, never to the notes.
const SKIP: &[&str] = &[
    ".DS_Store",
    ".obsidian",
    ".git",
    ".trash",
    ".Trash",
    "Thumbs.db",
    "desktop.ini",
    ".localized",
];

fn skipped(name: &str) -> bool {
    SKIP.contains(&name) || name.starts_with("~$")
}

/// Kinds of file someone can leave out to fit their account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Images,
    Pdfs,
    Media,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Images, Kind::Pdfs, Kind::Media];

    fn of(name: &str) -> Option<Kind> {
        let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "bmp" | "tif" | "tiff" | "svg" => {
                Some(Kind::Images)
            }
            "pdf" => Some(Kind::Pdfs),
            "mp4" | "mov" | "m4v" | "webm" | "mp3" | "m4a" | "wav" | "aac" | "flac" => {
                Some(Kind::Media)
            }
            _ => None,
        }
    }

    /// Plural noun for the sheet: "**80** images".
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Images => "images",
            Kind::Pdfs => "PDFs",
            Kind::Media => "videos and audio files",
        }
    }
}

/// What someone chose to leave out of this import.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Leave {
    pub kinds: Vec<Kind>,
    /// A folder inside the import, by its path relative to the import.
    pub folder: Option<String>,
}

/// Files and server usage that something accounts for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Share {
    pub files: usize,
    pub usage: u64,
}

impl Share {
    fn add(&mut self, usage: u64) {
        self.files += 1;
        self.usage += usage;
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImportPlan {
    /// What the sheet calls the import: a folder name, or "**n** items".
    pub subject: String,
    pub dest: Uuid,
    pub leave: Leave,
    /// Documents this import writes (new or changed), and their size on disk.
    pub files: usize,
    pub bytes: u64,
    pub new: usize,
    pub changed: usize,
    /// Documents already here with the same content.
    pub same: usize,
    /// Bytes the server will be asked to hold beyond today (writes plus fees).
    pub added_usage: u64,
    /// What each kind and each folder would add, whether or not it's left out.
    pub kinds: Vec<(Kind, Share)>,
    pub folders: Vec<(String, Share)>,
    /// What `leave` holds back.
    pub left: Share,
    pub skipped: usize,
    /// Distinct skipped names, for the sheet to list when there are few.
    pub skipped_names: Vec<String>,
    /// A top-level item already exists under `dest`, so this updates it.
    pub updates_existing: bool,
    pub unreadable: usize,
    /// The existing folder a single-folder import updates.
    pub root: Option<Uuid>,
    /// The account's usage and cap when planned; `None` when neither the server
    /// nor the last sync could say.
    pub room: Option<Room>,
}

/// Server usage plus what local changes will add once pushed, and the cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    pub used: u64,
    pub cap: u64,
}

impl Room {
    /// Asks the server, falling back to the last sync's numbers, then adds what
    /// local changes hold, at stored size. Edits count whole, so they overestimate.
    pub fn measure(core: &Lb, status: &Status) -> Option<Self> {
        let (server, cap) = match core.get_usage() {
            Ok(u) => (u.server_usage.exact, u.data_cap.exact),
            Err(_) => {
                let u = status.space_used.as_ref()?;
                (u.server_usage.exact, u.data_cap.exact)
            }
        };
        let pending: u64 = core
            .get_local_changes()
            .unwrap_or_default()
            .iter()
            .filter_map(|id| core.get_file_by_id(*id).ok())
            .map(|f| f.size_bytes + METADATA_FEE)
            .sum();
        Some(Self { used: server + pending, cap })
    }

    pub fn premium(&self) -> bool {
        self.cap > FREE_TIER_USAGE_SIZE
    }
}

impl ImportPlan {
    pub fn writes(&self) -> usize {
        self.new + self.changed
    }

    /// `(after, cap)` when this import would go over the account's cap.
    pub fn over_cap(&self) -> Option<(u64, u64)> {
        let r = self.room?;
        let after = r.used + self.added_usage;
        (after > r.cap).then_some((after, r.cap))
    }

    /// How far the whole import, nothing left out, goes over the cap.
    fn whole_excess(&self) -> Option<u64> {
        let r = self.room?;
        (r.used + self.added_usage + self.left.usage)
            .checked_sub(r.cap)
            .filter(|e| *e > 0)
    }

    pub fn whole_over_cap(&self) -> bool {
        self.whole_excess().is_some()
    }

    /// The smallest single folder whose absence brings the whole import under the cap.
    pub fn fitting_folder(&self) -> Option<(String, Share)> {
        let excess = self.whole_excess()?;
        self.folders
            .iter()
            .filter(|(_, s)| s.usage >= excess)
            .min_by_key(|(_, s)| s.usage)
            .cloned()
    }

    /// Kinds that, left out alone, bring the whole import under the cap.
    pub fn fitting_kinds(&self) -> Vec<(Kind, Share)> {
        let Some(excess) = self.whole_excess() else { return Vec::new() };
        self.kinds
            .iter()
            .filter(|(_, s)| s.usage >= excess)
            .copied()
            .collect()
    }
}

/// One document the job writes, kept for the walk that executes the plan.
enum Write {
    Create {
        name: String,
        parent: Parent,
        disk: PathBuf,
        bytes: u64,
    },
    Update {
        id: Uuid,
        disk: PathBuf,
        bytes: u64,
    },
    /// `top`: a folder the import names, not one inside it.
    Folder {
        name: String,
        parent: Parent,
        slot: usize,
        top: bool,
    },
}

/// A parent that exists now, or a folder the job creates earlier in the list.
#[derive(Clone, Copy)]
enum Parent {
    Existing(Uuid),
    Created(usize),
}

struct Walk<'a> {
    core: &'a Lb,
    plan: ImportPlan,
    writes: Vec<Write>,
    folders: usize,
    /// Paths of the folders inside the import that hold what's being visited.
    within: Vec<String>,
    /// Children of each existing folder, listed once.
    children: HashMap<Uuid, Vec<File>>,
    /// Folders on disk already walked, so a symlink back up doesn't loop.
    seen: HashSet<PathBuf>,
    /// A throwaway key for sizing documents the way the server stores them.
    key: AESKey,
}

/// What the server counts for `data`: compressed, then encrypted.
fn stored_size(key: &AESKey, data: &[u8]) -> u64 {
    compression_service::compress(data)
        .and_then(|c| symkey::encrypt(key, &c))
        .map_or(data.len(), |e| e.value.len()) as u64
}

impl Walk<'_> {
    fn child(&mut self, parent: Parent, name: &str) -> Option<File> {
        let Parent::Existing(id) = parent else { return None };
        let core = self.core;
        self.children
            .entry(id)
            .or_insert_with(|| core.get_children(&id).unwrap_or_default())
            .iter()
            .find(|f| f.name == name)
            .cloned()
    }

    fn share(list: &mut Vec<(String, Share)>, key: &str, usage: u64) {
        match list.iter_mut().find(|(k, _)| k == key) {
            Some((_, s)) => s.add(usage),
            None => {
                let mut s = Share::default();
                s.add(usage);
                list.push((key.to_owned(), s));
            }
        }
    }

    /// `inside` is the path of `disk`'s folder within the import (`""` for the
    /// import's own folder), `None` for a top-level item. `left` is true under a
    /// folder someone chose to leave out.
    fn visit(&mut self, disk: &Path, parent: Parent, inside: Option<&str>, left: bool) {
        let Some(name) = disk.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            self.plan.unreadable += 1;
            return;
        };
        if skipped(&name) {
            self.plan.skipped += 1;
            if !self.plan.skipped_names.contains(&name) {
                self.plan.skipped_names.push(name);
            }
            return;
        }
        let Ok(meta) = fs::metadata(disk) else {
            self.plan.unreadable += 1;
            return;
        };
        let existing = if left { None } else { self.child(parent, &name) };
        if inside.is_none() && existing.is_some() {
            self.plan.updates_existing = true;
            self.plan.root = existing.as_ref().filter(|f| f.is_folder()).map(|f| f.id);
        }

        if meta.is_dir() {
            if !fs::canonicalize(disk).is_ok_and(|c| self.seen.insert(c)) {
                self.plan.unreadable += 1;
                return;
            }
            let rel = match inside {
                None => String::new(),
                Some("") => name.clone(),
                Some(p) => format!("{p}/{name}"),
            };
            let left = left || (!rel.is_empty() && self.plan.leave.folder.as_ref() == Some(&rel));
            let here = match (left, existing.filter(|f| f.is_folder())) {
                (true, _) => parent,
                (false, Some(f)) => Parent::Existing(f.id),
                (false, None) => {
                    let slot = self.folders;
                    self.folders += 1;
                    self.plan.added_usage += METADATA_FEE;
                    let top = inside.is_none();
                    self.writes.push(Write::Folder { name, parent, slot, top });
                    Parent::Created(slot)
                }
            };
            let Ok(entries) = fs::read_dir(disk) else {
                self.plan.unreadable += 1;
                return;
            };
            let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            children.sort();
            let counted = !rel.is_empty();
            if counted {
                self.within.push(rel.clone());
            }
            for child in children {
                self.visit(&child, here, Some(&rel), left);
            }
            if counted {
                self.within.pop();
            }
            return;
        }

        let bytes = meta.len();
        let Ok(data) = fs::read(disk) else {
            self.plan.unreadable += 1;
            return;
        };
        let update = existing.filter(|f| f.is_document());
        if update.as_ref().is_some_and(|f| {
            self.core
                .read_document(f.id, false)
                .is_ok_and(|o| o == data)
        }) {
            self.plan.same += 1;
            return;
        }
        let stored = stored_size(&self.key, &data);
        // An edit costs what it grows by; the server allows a shrink at any usage.
        let usage = match &update {
            Some(f) => stored.saturating_sub(f.size_bytes),
            None => stored + METADATA_FEE,
        };
        let kind = Kind::of(&name);
        if let Some(k) = kind {
            match self.plan.kinds.iter_mut().find(|(x, _)| *x == k) {
                Some((_, s)) => s.add(usage),
                None => {
                    let mut s = Share::default();
                    s.add(usage);
                    self.plan.kinds.push((k, s));
                }
            }
        }
        for f in &self.within {
            Self::share(&mut self.plan.folders, f, usage);
        }
        if left || kind.is_some_and(|k| self.plan.leave.kinds.contains(&k)) {
            self.plan.left.add(usage);
            return;
        }
        self.plan.files += 1;
        self.plan.bytes += bytes;
        self.plan.added_usage += usage;
        match update {
            Some(f) => {
                self.plan.changed += 1;
                self.writes
                    .push(Write::Update { id: f.id, disk: disk.to_owned(), bytes });
            }
            None => {
                self.plan.new += 1;
                self.writes
                    .push(Write::Create { name, parent, disk: disk.to_owned(), bytes });
            }
        }
    }
}

fn walk<'a>(
    core: &'a Lb, paths: &[PathBuf], dest: Uuid, leave: Leave, room: Option<Room>,
) -> Walk<'a> {
    let subject = match paths {
        [one] => one
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        many => format!("{} items", many.len()),
    };
    let mut w = Walk {
        core,
        plan: ImportPlan { subject, dest, leave, room, ..Default::default() },
        writes: Vec::new(),
        folders: 0,
        within: Vec::new(),
        children: HashMap::new(),
        seen: HashSet::new(),
        key: symkey::generate_key(),
    };
    for p in paths {
        w.visit(p, Parent::Existing(dest), None, false);
    }
    w.plan.folders.sort_by(|a, b| a.0.cmp(&b.0));
    w
}

pub fn plan(
    core: &Lb, paths: &[PathBuf], dest: Uuid, leave: Leave, room: Option<Room>,
) -> ImportPlan {
    walk(core, paths, dest, leave, room).plan
}

/// The plan for `dest`, leaving out the smallest thing that makes it fit
/// when the whole import won't.
pub fn plan_to_fit(core: &Lb, paths: &[PathBuf], dest: Uuid, room: Option<Room>) -> ImportPlan {
    let whole = plan(core, paths, dest, Leave::default(), room);
    if whole.over_cap().is_none() {
        return whole;
    }
    let kind = whole
        .fitting_kinds()
        .into_iter()
        .min_by_key(|(_, s)| s.usage);
    let folder = whole.fitting_folder();
    let leave = match (kind, folder) {
        (Some((k, ks)), Some((_, fs))) if ks.usage <= fs.usage => {
            Leave { kinds: vec![k], folder: None }
        }
        (_, Some((f, _))) => Leave { kinds: vec![], folder: Some(f) },
        (Some((k, _)), None) => Leave { kinds: vec![k], folder: None },
        (None, None) => return whole,
    };
    plan(core, paths, dest, leave, room)
}

/// Plans on another thread: listing and comparing a big folder takes about a
/// second per 5,000 files. `leave: None` picks what to leave out so it fits.
pub fn plan_in_background(
    core: Lb, paths: Vec<PathBuf>, dest: Uuid, leave: Option<Leave>, status: Status,
    ctx: egui::Context,
) -> std::sync::mpsc::Receiver<ImportPlan> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let room = Room::measure(&core, &status);
        let plan = match leave {
            Some(leave) => plan(&core, &paths, dest, leave, room),
            None => plan_to_fit(&core, &paths, dest, room),
        };
        let _ = tx.send(plan);
        ctx.request_repaint();
    });
    rx
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Writing into the local account: create, encrypt, store.
    Importing,
    /// Waiting for sync to push what the import wrote.
    Uploading,
    Done,
}

/// Shared between the import thread and the UI.
#[derive(Debug)]
pub struct Progress {
    pub phase: Phase,
    pub done_files: usize,
    pub done_bytes: u64,
    /// The file being written or pushed, for the card's second line.
    pub current: Option<String>,
    pub failed: Vec<(String, String)>,
    /// The top-level folder this import made, once it exists.
    pub root: Option<Uuid>,
    /// Nothing was written: by the time it ran, the import no longer fit.
    pub refused: bool,
}

pub struct ImportJob {
    pub plan: ImportPlan,
    pub progress: Arc<Mutex<Progress>>,
    phase_started: Instant,
    /// Sizes of what the import wrote, to count bytes as they leave.
    written: Arc<Mutex<HashMap<Uuid, u64>>>,
    names: Arc<Mutex<HashMap<Uuid, String>>>,
    uploading: bool,
}

/// What the sidebar card draws this frame.
#[derive(Clone, Debug)]
pub struct JobView {
    pub phase: Phase,
    pub subject: String,
    pub done: usize,
    pub total: usize,
    pub fraction: f32,
    pub current: Option<String>,
    /// Seconds left, once there's enough history to guess.
    pub eta: Option<f64>,
    pub skipped: usize,
    pub failed: usize,
    /// The first few files that didn't import.
    pub failed_names: Vec<String>,
    pub refused: bool,
    pub same: usize,
    pub root: Option<Uuid>,
}

/// Before this much of a phase has passed, the card says it's estimating.
const ETA_WARMUP_SECS: f64 = 3.0;

impl ImportJob {
    /// Runs `plan` again where it writes, so it acts on the folder as it is now.
    /// Refuses, writing nothing, when that no longer fits the account.
    pub fn start(
        core: Lb, paths: Vec<PathBuf>, plan: ImportPlan, status: Status, ctx: egui::Context,
    ) -> Self {
        let progress = Arc::new(Mutex::new(Progress {
            phase: Phase::Importing,
            done_files: 0,
            done_bytes: 0,
            current: None,
            failed: Vec::new(),
            root: plan.root,
            refused: false,
        }));
        let written = Arc::new(Mutex::new(HashMap::new()));
        let names = Arc::new(Mutex::new(HashMap::new()));
        let job = Self {
            plan,
            progress: progress.clone(),
            phase_started: Instant::now(),
            written: written.clone(),
            names: names.clone(),
            uploading: false,
        };
        let (dest, leave) = (job.plan.dest, job.plan.leave.clone());
        std::thread::spawn(move || {
            let w = walk(&core, &paths, dest, leave, Room::measure(&core, &status));
            if w.plan.over_cap().is_some() {
                let mut p = progress.lock().unwrap();
                p.refused = true;
                p.phase = Phase::Done;
                drop(p);
                ctx.request_repaint();
                return;
            }
            run(&core, w.writes, &progress, &written, &names, &ctx);
            let mut p = progress.lock().unwrap();
            p.phase = Phase::Uploading;
            p.current = None;
            drop(p);
            ctx.request_repaint();
        });
        job
    }

    /// Follows sync once the local writes are in.
    pub fn tick(&mut self, status: &Status) {
        if self.progress.lock().unwrap().phase != Phase::Uploading {
            return;
        }
        if !self.uploading {
            self.uploading = true;
            self.phase_started = Instant::now();
        }
        let written = self.written.lock().unwrap();
        let dirty: HashSet<Uuid> = status.dirty_locally.iter().copied().collect();
        let left: Vec<&Uuid> = written.keys().filter(|id| dirty.contains(id)).collect();
        let mut p = self.progress.lock().unwrap();
        p.done_files = written.len() - left.len();
        p.done_bytes = written
            .iter()
            .filter(|(id, _)| !dirty.contains(id))
            .map(|(_, b)| b)
            .sum();
        p.current = status
            .pushing_files
            .iter()
            .find_map(|id| self.names.lock().unwrap().get(id).cloned());
        if left.is_empty() {
            p.phase = Phase::Done;
        }
    }

    pub fn view(&self) -> JobView {
        let p = self.progress.lock().unwrap();
        let total = self.plan.writes();
        let total_bytes = self.plan.bytes.max(1);
        let fraction = if total == 0 {
            1.0
        } else if p.phase == Phase::Importing {
            // Weighted by the measured file and byte costs of writing.
            let (a, b) = (MS_PER_FILE, MS_PER_MB / 1e6);
            let all = a * total as f64 + b * total_bytes as f64;
            ((a * p.done_files as f64 + b * p.done_bytes as f64) / all).min(1.0) as f32
        } else {
            // Uploads cost per file; bytes barely register.
            (p.done_files as f32 / total as f32).min(1.0)
        };
        let elapsed = self.phase_started.elapsed().as_secs_f64();
        let eta = (p.phase != Phase::Done && elapsed >= ETA_WARMUP_SECS && fraction > 0.01)
            .then(|| elapsed * (1.0 - fraction as f64) / fraction as f64);
        JobView {
            phase: p.phase,
            subject: self.plan.subject.clone(),
            done: p.done_files,
            total,
            fraction,
            current: p.current.clone(),
            eta,
            skipped: self.plan.skipped,
            failed: p.failed.len(),
            failed_names: p.failed.iter().map(|(n, _)| n.clone()).take(3).collect(),
            refused: p.refused,
            same: self.plan.same,
            root: p.root,
        }
    }
}

/// Import cost per file and per MB, measured on a 3,000-note vault (2026-11-04).
const MS_PER_FILE: f64 = 19.0;
const MS_PER_MB: f64 = 10.0;

fn run(
    core: &Lb, writes: Vec<Write>, progress: &Mutex<Progress>, written: &Mutex<HashMap<Uuid, u64>>,
    names: &Mutex<HashMap<Uuid, String>>, ctx: &egui::Context,
) {
    let mut made: Vec<Option<Uuid>> = Vec::new();
    let resolve = |made: &Vec<Option<Uuid>>, p: Parent| match p {
        Parent::Existing(id) => Some(id),
        Parent::Created(slot) => made.get(slot).copied().flatten(),
    };
    for w in writes {
        let (label, result) = match w {
            Write::Folder { name, parent, slot, top } => {
                let id = resolve(&made, parent)
                    .and_then(|p| core.create_file(&name, &p, FileType::Folder).ok())
                    .map(|f| f.id);
                if made.len() <= slot {
                    made.resize(slot + 1, None);
                }
                made[slot] = id;
                let mut p = progress.lock().unwrap();
                if top && p.root.is_none() {
                    p.root = id;
                }
                continue;
            }
            Write::Create { name, parent, disk, bytes } => {
                progress.lock().unwrap().current = Some(name.clone());
                let r = resolve(&made, parent)
                    .ok_or_else(|| "its folder wasn't created".to_owned())
                    .and_then(|p| {
                        let f = core
                            .create_file(&name, &p, FileType::Document)
                            .map_err(|e| e.to_string())?;
                        let data = fs::read(&disk).map_err(|e| e.to_string())?;
                        core.write_document(f.id, &data)
                            .map_err(|e| e.to_string())?;
                        Ok((f.id, bytes))
                    });
                (name, r)
            }
            Write::Update { id, disk, bytes } => {
                let name = disk
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                progress.lock().unwrap().current = Some(name.clone());
                let r = fs::read(&disk)
                    .map_err(|e| e.to_string())
                    .and_then(|d| core.write_document(id, &d).map_err(|e| e.to_string()))
                    .map(|_| (id, bytes));
                (name, r)
            }
        };
        let mut p = progress.lock().unwrap();
        match result {
            Ok((id, bytes)) => {
                p.done_files += 1;
                p.done_bytes += bytes;
                written.lock().unwrap().insert(id, bytes);
                names.lock().unwrap().insert(id, label);
            }
            Err(e) => p.failed.push((label, e)),
        }
        drop(p);
        ctx.request_repaint();
    }
}
