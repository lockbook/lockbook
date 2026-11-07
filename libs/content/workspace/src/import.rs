//! Folder and file import for the app. lb-rs plans and writes; this adds what the sheet says
//! about the plan and a job that reports its progress.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbErrKind;
pub use lb_rs::service::import_export::IgnoreReason;
use lb_rs::service::import_export::{
    self as lb_import, Action, EntryType, Existing, ImportStatus, ImportSummary,
};

/// lb-rs's plan with what the sheet calls it.
#[derive(Clone, Debug)]
pub struct ImportPlan {
    pub plan: lb_import::ImportPlan,
    /// What the sheet calls the import: a folder name, or "**n** items".
    pub subject: String,
    summary: ImportSummary,
}

impl ImportPlan {
    pub fn new(plan: lb_import::ImportPlan, subject: String) -> Self {
        let summary = plan.summary();
        Self { plan, subject, summary }
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
    core: Lb, paths: Vec<PathBuf>, dest: Uuid, ctx: egui::Context,
) -> std::sync::mpsc::Receiver<Result<ImportPlan, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let plan = core
            .plan_import(&paths, dest, Existing::Update)
            .map(|p| ImportPlan::new(p, subject(&paths)))
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
    /// Hidden files left behind, and files that couldn't be read.
    pub hidden: usize,
    pub unreadable: usize,
    pub failed: usize,
    /// The first few files that didn't import.
    pub failed_names: Vec<String>,
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
    /// Writes `plan` as the sheet showed it.
    pub fn start(core: Lb, plan: ImportPlan, ctx: egui::Context) -> Self {
        let progress = Arc::new(Mutex::new(Progress {
            phase: Phase::Importing,
            done_files: 0,
            done_bytes: 0,
            current: None,
            failed: Vec::new(),
            root: plan.root(),
        }));
        let job = Self { plan: plan.clone(), progress: progress.clone(), started: Instant::now() };
        std::thread::spawn(move || {
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
            hidden: self.plan.summary.hidden,
            unreadable: self.plan.summary.unreadable,
            failed: p.failed.len(),
            failed_names: p.failed.iter().take(3).cloned().collect(),
            same: self.plan.summary.same,
            root: p.root,
        }
    }
}
