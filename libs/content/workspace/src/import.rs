//! Folder and file import for the app. lb-rs plans and writes; this adds what the person chose to
//! skip, how the import sits against the account's cap, and a job that reports its progress.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::api::FREE_TIER_USAGE_SIZE;
use lb_rs::model::errors::LbErrKind;
use lb_rs::service::import_export::{
    self as lb_import, Action, EntryType, Existing, ImportStatus, ImportSummary,
};
pub use lb_rs::service::import_export::{DocTotals, IgnoreReason};
use lb_rs::subscribers::status::Status;

use crate::show::DocType;

/// Kinds of document someone can skip to fit their account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Images,
    Pdfs,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Images, Kind::Pdfs];

    /// The kind of a file named `name`, by the types the workspace opens it as,
    /// plus photo formats it can't open yet.
    pub fn of(name: &str) -> Option<Kind> {
        let ext = name.rsplit('.').next().unwrap_or_default().to_lowercase();
        if matches!(ext.as_str(), "heic" | "heif" | "tif" | "tiff") {
            return Some(Kind::Images);
        }
        match DocType::from_name(name) {
            DocType::Image | DocType::ImageUnsupported => Some(Kind::Images),
            DocType::PDF => Some(Kind::Pdfs),
            _ => None,
        }
    }
}

/// Plural noun for a kind: "Skip all images".
pub fn plural(kind: Kind) -> &'static str {
    match kind {
        Kind::Images => "images",
        Kind::Pdfs => "PDFs",
    }
}

/// Noun for `n` of a kind: "**1** image", "**80** images".
pub fn noun(kind: Kind, n: usize) -> &'static str {
    match (kind, n) {
        (Kind::Images, 1) => "image",
        (Kind::Pdfs, 1) => "PDF",
        _ => plural(kind),
    }
}

/// What each kind of document would add when nothing is skipped.
fn by_kind(plan: &lb_import::ImportPlan) -> Vec<(Kind, DocTotals)> {
    let mut totals: HashMap<Kind, DocTotals> = HashMap::new();
    for e in &plan.entries {
        let (Action::Create | Action::Replace(_), EntryType::Document { added_bytes, .. }) =
            (e.action, e.file_type)
        else {
            continue;
        };
        if let Some(kind) = Kind::of(&e.name()) {
            let total = totals.entry(kind).or_default();
            total.files += 1;
            total.added_bytes += added_bytes;
        }
    }
    Kind::ALL
        .iter()
        .filter_map(|k| totals.get(k).map(|s| (*k, *s)))
        .collect()
}

/// What someone chose to skip so an import fits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skip {
    pub kinds: Vec<Kind>,
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
            .map(|f| f.size_bytes)
            .sum();
        Some(Self { used: server + pending, cap })
    }

    pub fn premium(&self) -> bool {
        self.cap > FREE_TIER_USAGE_SIZE
    }
}

/// lb-rs's plan with the person's skips applied and the account's room beside it.
#[derive(Clone, Debug)]
pub struct ImportPlan {
    pub plan: lb_import::ImportPlan,
    /// What the sheet calls the import: a folder name, or "**n** items".
    pub subject: String,
    pub skip: Skip,
    /// The account's usage and cap when planned; `None` when neither the server
    /// nor the last sync could say.
    pub room: Option<Room>,
    /// What each kind would add, skipped or not.
    pub kinds: Vec<(Kind, DocTotals)>,
    summary: ImportSummary,
}

impl ImportPlan {
    pub fn new(plan: lb_import::ImportPlan, subject: String, room: Option<Room>) -> Self {
        let (kinds, summary) = (by_kind(&plan), plan.summary());
        Self { plan, subject, skip: Skip::default(), room, kinds, summary }
    }

    pub fn dest(&self) -> Uuid {
        self.plan.dest
    }

    pub fn summary(&self) -> &ImportSummary {
        &self.summary
    }

    /// Documents this import writes, new or changed.
    pub fn writes(&self) -> usize {
        self.summary.new + self.summary.changed
    }

    /// Marks what `skip` covers and counts again.
    pub fn skipping(&mut self, skip: Skip) {
        for e in &mut self.plan.entries {
            if let EntryType::Document { .. } = e.file_type {
                e.skip = Kind::of(&e.name()).is_some_and(|k| skip.kinds.contains(&k));
            }
        }
        self.skip = skip;
        self.summary = self.plan.summary();
    }

    pub fn toggle_kind(&mut self, kind: Kind) {
        let mut skip = self.skip.clone();
        match skip.kinds.iter().position(|k| *k == kind) {
            Some(i) => _ = skip.kinds.remove(i),
            None => skip.kinds.push(kind),
        }
        self.skipping(skip);
    }

    /// `(after, cap)` when this import would go over the account's cap.
    pub fn over_cap(&self) -> Option<(u64, u64)> {
        let r = self.room?;
        let after = r.used + self.summary.added_bytes;
        (after > r.cap).then_some((after, r.cap))
    }

    /// How far the whole import, nothing skipped, goes over the cap.
    fn whole_excess(&self) -> Option<u64> {
        let r = self.room?;
        (r.used + self.summary.added_bytes + self.summary.skipped.added_bytes)
            .checked_sub(r.cap)
            .filter(|e| *e > 0)
    }

    pub fn whole_over_cap(&self) -> bool {
        self.whole_excess().is_some()
    }

    /// Distinct names of what's ignored for `reason`, for the sheet to list when there are few.
    pub fn ignored_names(&self, reason: IgnoreReason) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for e in &self.plan.entries {
            if e.action == Action::Ignore(reason) && !names.contains(&e.name()) {
                names.push(e.name());
            }
        }
        names
    }

    /// Picked items that aren't files or folders, links mostly, which an import refuses.
    pub fn refused_names(&self) -> Vec<String> {
        self.plan
            .entries
            .iter()
            .filter(|e| e.action == Action::Ignore(IgnoreReason::NotAFile) && self.is_top(e))
            .map(|e| e.name())
            .collect()
    }

    /// Whether the walk found anything it could bring in or already has.
    pub fn found_anything(&self) -> bool {
        self.plan
            .entries
            .iter()
            .any(|e| !matches!(e.action, Action::Ignore(_)))
    }

    /// The existing folder a single-folder import updates.
    pub fn root(&self) -> Option<Uuid> {
        let mut tops = self.plan.entries.iter().filter(|e| self.is_top(e));
        match (tops.next(), tops.next()) {
            (Some(e), None) if e.file_type == EntryType::Folder => match e.action {
                Action::Noop(id) => Some(id),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_top(&self, e: &lb_import::Entry) -> bool {
        !e.disk_path.parent().is_some_and(|p| {
            self.plan
                .entries
                .iter()
                .any(|f| f.file_type == EntryType::Folder && f.disk_path == p)
        })
    }
}

fn subject(paths: &[PathBuf]) -> String {
    match paths {
        [one] => one
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        many => format!("{} items", many.len()),
    }
}

/// Plans on another thread: listing and comparing a big folder takes about a
/// second per 5,000 files.
pub fn plan_in_background(
    core: Lb, paths: Vec<PathBuf>, dest: Uuid, status: Status, ctx: egui::Context,
) -> std::sync::mpsc::Receiver<Result<ImportPlan, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let room = Room::measure(&core, &status);
        let plan = core
            .plan_import(&paths, dest, Existing::Update)
            .map(|p| ImportPlan::new(p, subject(&paths), room))
            .map_err(|e| e.to_string());
        let _ = tx.send(plan);
        ctx.request_repaint();
    });
    rx
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Writing into the local account: create, encrypt, store. Sync uploads after.
    Importing,
    Done,
}

/// Shared between the import thread and the UI.
#[derive(Debug)]
pub struct Progress {
    pub phase: Phase,
    pub done_files: usize,
    pub done_bytes: u64,
    /// The file being written, for the card's second line, and its size on disk.
    pub current: Option<(String, u64)>,
    /// Names of the files that weren't written.
    pub failed: Vec<String>,
    /// The top-level folder this import made or updates, once it exists.
    pub root: Option<Uuid>,
    /// Nothing was written: by the time it ran, the import no longer fit.
    pub refused: bool,
}

pub struct ImportJob {
    pub plan: ImportPlan,
    pub progress: Arc<Mutex<Progress>>,
    started: Instant,
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
    /// Documents the person chose to skip, and their kind when they all share one.
    pub chosen: usize,
    pub chosen_kind: Option<Kind>,
    /// Hidden files left behind, and files that couldn't be read.
    pub hidden: usize,
    pub unreadable: usize,
    pub failed: usize,
    /// The first few files that didn't import.
    pub failed_names: Vec<String>,
    pub refused: bool,
    pub same: usize,
    pub root: Option<Uuid>,
}

/// Import cost per file and per MB, measured on a 3,000-note vault (2026-11-04).
const MS_PER_FILE: f64 = 19.0;
const MS_PER_MB: f64 = 10.0;

fn run(core: &Lb, plan: &ImportPlan, progress: &Mutex<Progress>, ctx: &egui::Context) {
    let dest = plan.dest();
    // `ImportStatus` names each entry by its disk path as displayed.
    let sizes: HashMap<String, u64> = plan
        .plan
        .entries
        .iter()
        .filter_map(|e| match e.file_type {
            EntryType::Document { disk_bytes, .. } => {
                Some((e.disk_path.display().to_string(), disk_bytes))
            }
            EntryType::Folder => None,
        })
        .collect();
    let status = |s: ImportStatus| {
        let mut p = progress.lock().unwrap();
        match s {
            ImportStatus::CalculatedTotal(_) => {}
            ImportStatus::StartingItem(path) => {
                p.current = sizes.get(&path).map(|bytes| {
                    let name = Path::new(&path).file_name().unwrap_or_default();
                    (name.to_string_lossy().into_owned(), *bytes)
                });
            }
            ImportStatus::FinishedItem(file) => {
                if file.is_folder() {
                    if file.parent == dest && p.root.is_none() {
                        p.root = Some(file.id);
                    }
                } else if let Some((_, bytes)) = p.current {
                    p.done_files += 1;
                    p.done_bytes += bytes;
                }
            }
        }
        drop(p);
        ctx.request_repaint();
    };
    if let Err(err) = core.import_planned(&plan.plan, &status) {
        let mut p = progress.lock().unwrap();
        p.failed = match err.kind {
            LbErrKind::ImportFailed { paths, .. } => paths
                .iter()
                .map(|path| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            _ => vec![plan.subject.clone()],
        };
    }
}

/// Before this much time has passed, the card says it's estimating.
const ETA_WARMUP_SECS: f64 = 3.0;

impl ImportJob {
    /// Writes `plan` as the sheet showed it. Refuses, writing nothing, when it no longer fits
    /// the account.
    pub fn start(core: Lb, plan: ImportPlan, status: Status, ctx: egui::Context) -> Self {
        let progress = Arc::new(Mutex::new(Progress {
            phase: Phase::Importing,
            done_files: 0,
            done_bytes: 0,
            current: None,
            failed: Vec::new(),
            root: plan.root(),
            refused: false,
        }));
        let job = Self { plan: plan.clone(), progress: progress.clone(), started: Instant::now() };
        std::thread::spawn(move || {
            let room = Room::measure(&core, &status);
            if room.is_some_and(|r| r.used + plan.summary.added_bytes > r.cap) {
                let mut p = progress.lock().unwrap();
                p.refused = true;
                p.phase = Phase::Done;
                drop(p);
                ctx.request_repaint();
                return;
            }
            run(&core, &plan, &progress, &ctx);
            let mut p = progress.lock().unwrap();
            p.phase = Phase::Done;
            p.current = None;
            drop(p);
            ctx.request_repaint();
        });
        job
    }

    pub fn view(&self) -> JobView {
        let p = self.progress.lock().unwrap();
        let total = self.plan.writes();
        let total_bytes = self.plan.summary.disk_bytes.max(1);
        let fraction = if total == 0 {
            1.0
        } else {
            // Weighted by the measured file and byte costs of writing.
            let (a, b) = (MS_PER_FILE, MS_PER_MB / 1e6);
            let all = a * total as f64 + b * total_bytes as f64;
            ((a * p.done_files as f64 + b * p.done_bytes as f64) / all).min(1.0) as f32
        };
        let elapsed = self.started.elapsed().as_secs_f64();
        let eta = (p.phase != Phase::Done && elapsed >= ETA_WARMUP_SECS && fraction > 0.01)
            .then(|| elapsed * (1.0 - fraction as f64) / fraction as f64);
        JobView {
            phase: p.phase,
            subject: self.plan.subject.clone(),
            done: p.done_files,
            total,
            fraction,
            current: p.current.as_ref().map(|(name, _)| name.clone()),
            eta,
            chosen: self.plan.summary.skipped.files,
            chosen_kind: match self.plan.skip.kinds.as_slice() {
                [kind] => Some(*kind),
                _ => None,
            },
            hidden: self.plan.summary.hidden,
            unreadable: self.plan.summary.unreadable,
            failed: p.failed.len(),
            failed_names: p.failed.iter().take(3).cloned().collect(),
            refused: p.refused,
            same: self.plan.summary.same,
            root: p.root,
        }
    }
}
