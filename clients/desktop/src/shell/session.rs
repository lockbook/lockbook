//! Core load + signed-in workspace session for the product shell.
//!
//! ## Session shape (account lifecycle)
//!
//! | State | `Lb` | `FileCache` / `Workspace` | UI |
//! |-------|------|---------------------------|-----|
//! | `Loading` (cold) | none yet | none | empty chrome (no plate) |
//! | `Loading` (onboard) | none | none | boot plate + spinner |
//! | `Error` | none | none | error boot |
//! | `SignedOut` | yes (empty account) | **none** | onboard only |
//! | `Ready` | yes (inside `Workspace`) | yes (`Workspace.files`) | shell chrome |
//!
//! Cold start only inits core + opens an existing account (no network sync).
//! Create/import still syncs on a worker with a plate.
//!
//! **Logout** wipes the data dir and `process::exit(0)`. In-process re-init
//! while workspace/lb workers still hold the DB is not supported.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use egui::Context;
use lb::Uuid;
use lb::blocking::Lb;
use lb::model::api::SubscriptionInfo;
use lb::model::core_config::Config;
use lb::model::errors::LbErrKind;
use lb::subscribers::status::Status;
use workspace_rs::file_cache::FileCache;
use workspace_rs::workspace::Workspace;

pub enum CoreLoad {
    Ready {
        core: Lb,
        files: FileCache,
        sub_info: Option<SubscriptionInfo>,
    },
    SignedOut {
        core: Lb,
    },
    /// Create/import/sync failed — keep `core` for retry (may already hold keys).
    OnboardFailed {
        core: Lb,
        err: String,
    },
    Failed(String),
}

/// Shared boot status for [`Session::Loading`] (worker may update mid-flight).
pub type LoadStatus = Arc<Mutex<String>>;

pub fn load_status(msg: impl Into<String>) -> LoadStatus {
    Arc::new(Mutex::new(msg.into()))
}

pub fn set_load_status(status: &LoadStatus, msg: impl Into<String>) {
    if let Ok(mut g) = status.lock() {
        *g = msg.into();
    }
}

pub fn read_load_status(status: &LoadStatus) -> String {
    status
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| "Loading…".into())
}

pub struct Ready {
    pub workspace: Workspace,
    pub expanded: HashSet<Uuid>,
    /// Tree / list selection (highlight, multi-select, context-menu targets).
    /// Not keyboard focus and not create/import destination — those follow the
    /// open tab. Right-clicking a folder updates this without opening a file.
    pub cursor: Option<Uuid>,
    /// Shift-range anchor (Finder-style).
    pub anchor: Option<Uuid>,
    /// Multi-select set (includes cursor when set).
    pub selected: HashSet<Uuid>,
    pub status: Status,
    pub status_msg: String,
    pub syncing: bool,
    pub pinned: Vec<Uuid>,
    pub sub_info: Option<SubscriptionInfo>,
    /// Flattened visible tree order (Shift-range select).
    pub nav_order: Vec<Uuid>,
    /// Bumped when [`super::ops::note_files_changed`] runs.
    /// Sidebar list panes (Recents / Shared) rebuild derived rows only when this changes.
    pub files_epoch: u64,
    /// Pending Files-tree center-scroll (tab open / reveal). Held until the row
    /// is in the flat walk, then consumed by the tree scroll animator.
    pub tree_scroll: Option<Uuid>,
    /// Local username roster for share field (instant; no UI-thread network).
    pub known_usernames: Vec<String>,
}

impl Ready {
    #[tracing::instrument(name = "Ready::new", level = "trace", skip_all)]
    pub fn new(
        core: Lb, files: FileCache, ctx: &Context, sub_info: Option<SubscriptionInfo>,
    ) -> Self {
        let file_cache = Arc::new(RwLock::new(files));
        let root = file_cache.read().unwrap().root.id;
        let mut workspace = Workspace::new(&core, ctx, false, true, Some(file_cache));
        workspace.show_tabs = false;
        workspace.sidebar_open = true;

        let status = core.status();
        let pinned = core.list_pinned().unwrap_or_default();
        let known_usernames = core.known_usernames().unwrap_or_default();
        Self {
            workspace,
            expanded: [root].into_iter().collect(),
            cursor: None,
            anchor: None,
            selected: HashSet::new(),
            status,
            status_msg: "Up to date".into(),
            syncing: false,
            pinned,
            sub_info,
            nav_order: Vec::new(),
            // Start at 1 so empty caches (epoch 0) rebuild on first paint.
            files_epoch: 1,
            tree_scroll: None,
            known_usernames,
        }
    }

    /// Request an animated center-scroll once `id` is visible in the flat walk.
    pub fn request_tree_scroll(&mut self, id: Uuid) {
        self.tree_scroll = Some(id);
    }

    /// Refresh local username cache (share open / after successful invite).
    pub fn refresh_known_usernames(&mut self) {
        if let Ok(names) = self.workspace.core.known_usernames() {
            self.known_usernames = names;
        }
    }

    pub fn remember_username(&mut self, username: &str) {
        let u = username.trim();
        if u.is_empty() {
            return;
        }
        if !self
            .known_usernames
            .iter()
            .any(|k| k.eq_ignore_ascii_case(u))
        {
            self.known_usernames.push(u.to_owned());
        }
    }

    /// Snapshot `core.status()` into view state.
    ///
    /// Point read of the status
    /// subscriber. **Not** every frame — use `Event::StatusUpdated` or after
    /// an op that may change dirty/sync flags.
    pub fn refresh_status(&mut self) {
        self.status = self.workspace.core.status();
        self.syncing = self.status.syncing;
        self.status_msg = self
            .status
            .msg()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if self.syncing {
                    "Syncing…".into()
                } else if self.status.offline {
                    "Offline".into()
                } else if !self.status.dirty_locally.is_empty() {
                    format!("{} changes unsynced", self.status.dirty_locally.len())
                } else {
                    "Up to date".into()
                }
            });
    }

    pub fn select_only(&mut self, id: Uuid) {
        self.cursor = Some(id);
        self.anchor = Some(id);
        self.selected.clear();
        self.selected.insert(id);
    }

    pub fn toggle_select(&mut self, id: Uuid) {
        if self.selected.contains(&id) {
            self.selected.remove(&id);
            if self.cursor == Some(id) {
                self.cursor = self.selected.iter().next().copied();
            }
        } else {
            self.selected.insert(id);
            self.cursor = Some(id);
            self.anchor = Some(id);
        }
    }

    /// Select contiguous visible range from anchor to `id` (using `nav_order`).
    pub fn select_range_to(&mut self, id: Uuid) {
        let anchor = self.anchor.or(self.cursor).unwrap_or(id);
        self.cursor = Some(id);
        if self.nav_order.is_empty() {
            self.select_only(id);
            return;
        }
        let a = self.nav_order.iter().position(|x| *x == anchor);
        let b = self.nav_order.iter().position(|x| *x == id);
        match (a, b) {
            (Some(i), Some(j)) => {
                let (lo, hi) = if i <= j { (i, j) } else { (j, i) };
                self.selected = self.nav_order[lo..=hi].iter().copied().collect();
            }
            _ => self.select_only(id),
        }
    }

    pub fn selection_vec(&self) -> Vec<Uuid> {
        if self.selected.is_empty() {
            self.cursor.into_iter().collect()
        } else {
            self.selected.iter().copied().collect()
        }
    }
}

/// Why we are in [`Session::Loading`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    /// App launch: open local core / existing account. No plate, no sync.
    Cold,
    /// Create / import: plate + status; worker may sync.
    Onboard,
}

pub enum Session {
    Loading { kind: LoadKind, status: LoadStatus, rx: Receiver<CoreLoad> },
    Error(String),
    SignedOut { core: Lb },
    Ready(Box<Ready>),
}

impl Session {
    /// Cold launch: init core, open account if present. No network sync.
    pub fn start(ctx: &Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let cfg = Config::ui_config("egui");
            let core = match Lb::init(cfg) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(CoreLoad::Failed(format!("{e:?}")));
                    ctx.request_repaint();
                    return;
                }
            };
            let signed_in = match core.get_account() {
                Ok(_) => true,
                Err(e) if matches!(e.kind, LbErrKind::AccountNonexistent) => false,
                Err(e) => {
                    let _ = tx.send(CoreLoad::Failed(format!("{e:?}")));
                    ctx.request_repaint();
                    return;
                }
            };
            if signed_in {
                // Local open only — no sync. Create/import syncs on its own worker.
                match FileCache::new(&core) {
                    Ok(files) => {
                        let _ = tx.send(prepare_ready(core, files));
                    }
                    Err(e) => {
                        let _ = tx.send(CoreLoad::Failed(format!("{e:?}")));
                    }
                }
            } else {
                let _ = tx.send(CoreLoad::SignedOut { core });
            }
            ctx.request_repaint();
        });
        Self::Loading { kind: LoadKind::Cold, status: load_status(""), rx }
    }

    /// Poll a background load. Returns an onboard error string when sign-in
    /// failed but we still have a usable `SignedOut` core to retry with.
    pub fn poll(&mut self, ctx: &Context) -> Option<String> {
        let Session::Loading { rx, .. } = self else {
            return None;
        };
        let load = rx.try_recv().ok()?;
        match load {
            CoreLoad::Ready { core, files, sub_info } => {
                *self = Session::Ready(Box::new(Ready::new(core, files, ctx, sub_info)));
                None
            }
            CoreLoad::SignedOut { core } => {
                *self = Session::SignedOut { core };
                None
            }
            CoreLoad::OnboardFailed { core, err } => {
                *self = Session::SignedOut { core };
                Some(err)
            }
            CoreLoad::Failed(e) => {
                *self = Session::Error(e);
                None
            }
        }
    }

    pub fn ready(&self) -> Option<&Ready> {
        match self {
            Session::Ready(r) => Some(r),
            _ => None,
        }
    }

    pub fn ready_mut(&mut self) -> Option<&mut Ready> {
        match self {
            Session::Ready(r) => Some(r),
            _ => None,
        }
    }

    pub fn signed_out_core(&self) -> Option<&Lb> {
        match self {
            Session::SignedOut { core } => Some(core),
            _ => None,
        }
    }
}

pub(crate) fn prepare_ready(core: Lb, files: FileCache) -> CoreLoad {
    let sub_info = core.get_subscription_info().ok().flatten();
    CoreLoad::Ready { core, files, sub_info }
}
