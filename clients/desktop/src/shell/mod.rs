//! Product shell — design-system chrome + live Workspace.
//!
//! Host: `cargo run -p lockbook-desktop`.

// Public so `components::domain` can queue intents / read session without cycles
// at the crate-type boundary (same crate; reverse edge is shell → components).
mod account_plan;
pub mod action;
mod apply;
mod apply_account;
mod apply_onboard;
mod apply_share;
mod editor;
pub mod ops;
pub mod prefs;
pub mod session;
pub mod settings;
mod settings_account;
mod sheets;
mod sheet_share;
mod sheet_create;
mod sheet_folder;
mod sheet_onboard;
pub mod sidebar;
pub mod titlebar;
pub mod toasts;

pub use action::{Modal, SidebarPane};

// Domain surfaces live in the component library; re-export for shell-local paths.
pub use crate::components::domain::{footer, sync_dots, tabs, tree};

// OnboardMode / SettingsCat / Ready are used via module paths; re-export only what
// other crates/bin need from `shell::`.

use std::sync::{Arc, Mutex};

use egui::{Area, CentralPanel, Frame, Id, Order, SidePanel};
use lb::Uuid;
use lb::service::events::{self, Event};
use workspace_rs::file_cache::FilesExt;

use crate::components::{self, Space, Spacer, ThemeFamily, Theme, TypeRole, handle_toggle_shortcut, set_mode_preference, set_theme_family, sheet_panel_fit, ThemeExt};
use crate::settings::Settings;

use action::Action as A;
use prefs::AccountPanel;
use session::{Ready as ReadyState, Session};
use toasts::ToastHost;

/// Background `Lb::debug_info` JSON for Settings → Debug (support copy-paste).
#[derive(Clone, Default)]
pub enum DebugInfoCache {
    #[default]
    Idle,
    Loading,
    Ready(String),
}

/// Background `export_account_qr` PNG for Settings → Account / AccountQr sheet.
#[derive(Clone, Default)]
pub enum AccountQrCache {
    #[default]
    Idle,
    Loading,
    Ready(Vec<u8>),
    Err(String),
}

/// Full-window product shell.
pub struct ShellApp {
    pub session: Session,
    pub sidebar_open: bool,
    pub pane: SidebarPane,
    pub modal: Option<Modal>,
    /// Desktop chrome — sole store for `egui/settings.json` (no ShellPrefs mirror).
    pub settings: Settings,
    /// Settings → Account in-content panel (session only; not on disk).
    pub account_panel: AccountPanel,
    /// Settings → Debug: expand JSON dump (session only).
    pub debug_info_revealed: bool,
    pub phrase_cache: Option<String>,
    /// Shared with the debug-info worker thread.
    pub debug_info: Arc<Mutex<DebugInfoCache>>,
    /// Shared with the account-QR worker (mobile sign-in).
    pub account_qr: Arc<Mutex<AccountQrCache>>,
    queue: Vec<A>,
    lb_rx: Option<events::Receiver<Event>>,
    /// Derived Recents rows; rebuilt when [`Ready::files_epoch`] changes.
    pub recents_cache: RecentsCache,
    /// Derived Shared lists; same epoch rule.
    pub shared_cache: SharedCache,
    theme_applied: bool,
    /// macOS: one-shot NSWindow.isMovable=false (see [`macos_window`]).
    /// Set by the host after window creation.
    #[cfg(target_os = "macos")]
    pub macos_window_tweaked: bool,
    /// Debounced per-file sync dots (Files tree only).
    pub sync_dots: sync_dots::SyncDots,
    /// Footer stable message + spin-after-tap.
    pub sync_footer: footer::SyncFooterState,
    /// `LOCKBOOK_SESSION_STRESS=1` — auto logout when Ready, re-import when
    /// SignedOut (needs `LOCKBOOK_STRESS_KEY`). Dev-only crash harness.
    session_stress: Option<SessionStress>,
    /// Transient errors / short notices (import, rename, workspace failures).
    pub toasts: ToastHost,
}

/// Dev harness: bounce Ready ↔ SignedOut without clicking UI.
struct SessionStress {
    /// Frames to wait in the current state before acting.
    settle_frames: u32,
    cycles_left: u32,
    last_kind: StressKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StressKind {
    Ready,
    SignedOut,
    Loading,
}

/// Cached Recents list (full-cache walk + path crumbs only on epoch bump).
#[derive(Default)]
pub struct RecentsCache {
    pub epoch: u64,
    pub rows: Vec<(Uuid, String, i64, String, bool)>,
}

/// Cached Shared-with-me list (pending share roots only).
#[derive(Default)]
pub struct SharedCache {
    pub epoch: u64,
    /// `(id, name, shared_by, is_folder)`
    pub pending: Vec<(Uuid, String, String, bool)>,
}

impl Default for ShellApp {
    fn default() -> Self {
        let settings = Settings::load();
        let sidebar_open = !settings.zen_mode;
        Self {
            session: Session::Error("not started".into()),
            sidebar_open,
            pane: SidebarPane::Files,
            modal: None,
            settings,
            account_panel: AccountPanel::Closed,
            debug_info_revealed: false,
            phrase_cache: None,
            debug_info: Arc::new(Mutex::new(DebugInfoCache::Idle)),
            account_qr: Arc::new(Mutex::new(AccountQrCache::Idle)),
            queue: Vec::new(),
            lb_rx: None,
            recents_cache: RecentsCache::default(),
            shared_cache: SharedCache::default(),
            theme_applied: false,
            #[cfg(target_os = "macos")]
            macos_window_tweaked: false,
            sync_dots: sync_dots::SyncDots::default(),
            sync_footer: footer::SyncFooterState::default(),
            session_stress: std::env::var_os("LOCKBOOK_SESSION_STRESS").map(|_| SessionStress {
                settle_frames: 30,
                cycles_left: std::env::var("LOCKBOOK_STRESS_CYCLES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5),
                last_kind: StressKind::Loading,
            }),
            toasts: ToastHost::default(),
        }
    }
}

impl ShellApp {
    pub fn close_account_panel(&mut self) {
        self.account_panel = AccountPanel::Closed;
    }

    /// Dev-gated renderer extras (`DEV_USERS`).
    pub fn is_dev(&self) -> bool {
        self.session
            .ready()
            .map(|r| crate::DEV_USERS.contains(&r.workspace.account.username.as_str()))
            .unwrap_or(false)
    }

    /// Auto logout / re-import loop for crash hunting (`LOCKBOOK_SESSION_STRESS=1`).
    fn session_stress_tick(&mut self, ctx: &egui::Context) {
        let Some(stress) = self.session_stress.as_mut() else {
            return;
        };
        if stress.cycles_left == 0 {
            eprintln!("[session-stress] done");
            self.session_stress = None;
            return;
        }

        let kind = match &self.session {
            Session::Ready(_) => StressKind::Ready,
            Session::SignedOut { .. } => StressKind::SignedOut,
            Session::Loading { .. } => StressKind::Loading,
            Session::Error(e) => {
                eprintln!("[session-stress] session error: {e}");
                self.session_stress = None;
                return;
            }
        };

        if kind != stress.last_kind {
            stress.last_kind = kind;
            stress.settle_frames = 45; // ~0.75s at 60Hz
            eprintln!(
                "[session-stress] state={:?} cycles_left={}",
                match kind {
                    StressKind::Ready => "Ready",
                    StressKind::SignedOut => "SignedOut",
                    StressKind::Loading => "Loading",
                },
                stress.cycles_left
            );
            return;
        }

        if stress.settle_frames > 0 {
            stress.settle_frames -= 1;
            ctx.request_repaint();
            return;
        }

        match kind {
            StressKind::Ready => {
                eprintln!("[session-stress] → logout");
                stress.cycles_left = stress.cycles_left.saturating_sub(1);
                // Skip UI ack; call the same door as ConfirmLogout after ack.
                self.account_panel = AccountPanel::Logout { acked: true };
                apply::apply(self, ctx, A::ConfirmLogout);
            }
            StressKind::SignedOut => {
                let key = std::env::var("LOCKBOOK_STRESS_KEY").unwrap_or_default();
                if key.trim().is_empty() {
                    eprintln!(
                        "[session-stress] SignedOut — set LOCKBOOK_STRESS_KEY to re-import, or stop"
                    );
                    self.session_stress = None;
                    return;
                }
                eprintln!("[session-stress] → import + promote");
                // Drive onboard import via actions.
                apply::apply(self, ctx, A::OnboardSetMode(action::OnboardMode::Import));
                if let Some(Modal::Onboard { import_kind, compact, words, .. }) = &mut self.modal {
                    *import_kind = action::OnboardImportKind::CompactKey;
                    *compact = key.trim().to_owned();
                    words.fill(String::new());
                }
                apply::apply(self, ctx, A::OnboardSubmit { show_error: true });
            }
            StressKind::Loading => {
                ctx.request_repaint();
            }
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        components::install(ctx);
        // F2 = space overlay (rename is context menu, so no conflict).
        handle_toggle_shortcut(ctx);
        self.apply_theme_once(ctx);

        if matches!(&self.session, Session::Error(s) if s == "not started") {
            self.session = Session::start(ctx);
        }
        if let Some(err) = self.session.poll(ctx) {
            // Sign-in worker failed; session is SignedOut again — show onboard error.
            self.modal = Some(Modal::Onboard {
                mode: action::OnboardMode::Import,
                uname: String::new(),
                uname_lookup: action::OnboardLookup::Idle,
                uname_lookup_for: String::new(),
                import_kind: action::OnboardImportKind::default(),
                compact: String::new(),
                words: action::empty_phrase_words(),
                busy: false,
                err: Some(err),
            });
        }
        if let Session::Ready(r) = &self.session {
            if self.lb_rx.is_none() {
                self.lb_rx = Some(r.workspace.core.subscribe());
            }
        }
        self.drain_events();
        self.process_keys(ctx);
        self.process_drops(ctx);

        // Close window: save workspace tabs
        if ctx.input(|i| i.viewport().close_requested()) {
            if let Some(r) = self.session.ready_mut() {
                r.workspace.save_all_tabs();
            }
        }

        let t = ctx.get_lb_theme();
        let mut queue = std::mem::take(&mut self.queue);

        match &self.session {
            Session::Loading { kind: session::LoadKind::Cold, .. } => {
                // Opening local account — empty chrome, no plate / no spinner.
                ctx.request_repaint();
                CentralPanel::default()
                    .frame(Frame::new().fill(t.neutral_bg()).inner_margin(0.0))
                    .show(ctx, |_| {});
                titlebar::show(self, ctx, &t, &mut queue);
                for a in queue {
                    apply::apply(self, ctx, a);
                }
                return;
            }
            Session::Loading { kind: session::LoadKind::Onboard, status, .. } => {
                let msg = session::read_load_status(status);
                boot_screen(ctx, &t, &msg, true);
                ctx.request_repaint();
                titlebar::show(self, ctx, &t, &mut queue);
                for a in queue {
                    apply::apply(self, ctx, a);
                }
                return;
            }
            Session::Error(msg) => {
                boot_screen(ctx, &t, msg, false);
                titlebar::show(self, ctx, &t, &mut queue);
                for a in queue {
                    apply::apply(self, ctx, a);
                }
                return;
            }
            Session::SignedOut { .. } => {
                // Offer onboard immediately
                if self.modal.is_none() {
                    self.modal = Some(Modal::Onboard {
                        mode: action::OnboardMode::Choice,
                        uname: String::new(),
                        uname_lookup: action::OnboardLookup::Idle,
                        uname_lookup_for: String::new(),
                        import_kind: action::OnboardImportKind::default(),
                        compact: String::new(),
                        words: action::empty_phrase_words(),
                        busy: false,
                        err: None,
                    });
                }
            }
            Session::Ready(_) => {}
        }

        if self.modal.is_some() {
            sheets::show_modals(self, ctx, &t, &mut queue);
        }

        let show_side =
            matches!(self.session, Session::Ready(_)) && self.sidebar_open && !self.defaults_zen();

        // Soften resize stroke + shrink grab (scroll bar shares the right edge).
        let resize_style =
            if show_side { Some(sidebar::begin_resize_style(ctx, &t)) } else { None };
        SidePanel::left(sidebar::PANEL_ID)
            .resizable(true)
            .default_width(300.0)
            .width_range(sidebar::WIDTH_MIN..=sidebar::WIDTH_MAX)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(t.neutral_bg_secondary())
                    .inner_margin(0.0),
            )
            .show_animated(ctx, show_side, |ui| {
                // Do **not** `ui.disable()` under a sheet: egui fades disabled paint
                // (sticky elevated plates look transparent). Input is blocked by the
                // full-window dim + Foreground sheet; keys are gated in `process_keys`.
                sidebar::show(self, ui, &t, &mut queue);
            });
        if let Some(saved) = resize_style {
            sidebar::end_resize_style(ctx, saved);
        }

        CentralPanel::default()
            .frame(Frame::new().fill(t.neutral_bg()).inner_margin(0.0))
            .show(ctx, |ui| {
                if matches!(self.session, Session::Ready(_)) {
                    editor::show(self, ui, &t, &mut queue);
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            TypeRole::Body
                                .rich("Create or import an account to continue.")
                                .color(t.neutral_fg_secondary()),
                        );
                    });
                }
            });

        // SidePanel's content-side grab is covered by CentralPanel — reclaim it.
        // Separator starts below the tab strip when tabs are open; full height
        // when the strip is hidden (workspace flush to the top).
        if show_side {
            let strip_h = self
                .session
                .ready()
                .map(|r| if r.workspace.tab_strip.is_empty() { 0.0 } else { titlebar::HEADER_H })
                .unwrap_or(0.0);
            sidebar::resize_over_workspace(ctx, &t, strip_h);
        }

        titlebar::show(self, ctx, &t, &mut queue);

        for a in queue {
            apply::apply(self, ctx, a);
        }

        self.session_stress_tick(ctx);

        // Do **not** call `core.status()` every frame.

        // After apply so same-frame failures (rename, share, …) paint this pass.
        self.toasts.show(ctx, &t);
    }

    fn defaults_zen(&self) -> bool {
        self.settings.zen_mode
    }

    /// Rebuild Recents rows only when the file cache epoch advanced.
    pub(crate) fn ensure_recents_cache(&mut self) {
        let Some(ready) = self.session.ready() else {
            return;
        };
        let epoch = ready.files_epoch;
        if self.recents_cache.epoch == epoch {
            return;
        }
        let pinned = ready.pinned.clone();
        let mut rows = {
            let files = ready.workspace.files.read().unwrap();
            files
                .iter_files()
                .filter(|f| f.file_type == lb::model::file_metadata::FileType::Document)
                .map(|f| {
                    let crumbs = {
                        let mut parts = Vec::new();
                        let mut cur = Some(f.parent);
                        while let Some(c) = cur {
                            if let Some(p) = files.get_by_id(c) {
                                if p.is_root() {
                                    break;
                                }
                                parts.push(p.name.clone());
                                cur = if p.parent == p.id { None } else { Some(p.parent) };
                            } else {
                                break;
                            }
                        }
                        parts.reverse();
                        parts.join(" / ")
                    };
                    (f.id, f.name.clone(), f.last_modified as i64, crumbs, pinned.contains(&f.id))
                })
                .collect::<Vec<_>>()
        };
        rows.sort_by_key(|d| std::cmp::Reverse(d.2));
        self.recents_cache = RecentsCache { epoch, rows };
    }

    pub(crate) fn ensure_shared_cache(&mut self) {
        let Some(ready) = self.session.ready() else {
            return;
        };
        let epoch = ready.files_epoch;
        if self.shared_cache.epoch == epoch {
            return;
        }
        let pending = {
            let files = ready.workspace.files.read().unwrap();
            // Pending share roots only — not yet organized into the user's tree.
            files
                .shared_roots
                .iter()
                .map(|f| {
                    let from = f
                        .shares
                        .first()
                        .map(|s| s.shared_by.clone())
                        .unwrap_or_else(|| "someone".into());
                    (f.id, f.name.clone(), from, f.is_folder())
                })
                .collect::<Vec<_>>()
        };
        self.shared_cache = SharedCache { epoch, pending };
    }

    fn apply_theme_once(&mut self, ctx: &egui::Context) {
        if self.theme_applied {
            return;
        }
        self.theme_applied = true;
        set_mode_preference(ctx, self.settings.theme_mode);
        let fam = ThemeFamily::ALL
            .iter()
            .find(|f| f.name() == self.settings.theme_name)
            .copied()
            .unwrap_or_default();
        set_theme_family(ctx, fam);
    }

    fn drain_events(&mut self) {
        use events::broadcast::error::TryRecvError;
        let Some(rx) = &mut self.lb_rx else { return };
        // Full `FileCache::new` is heavier and runs on workspace `file_cache_updated`
        // (and explicit ops) — not on every status tick.
        let mut status_updated = false;
        loop {
            match rx.try_recv() {
                Ok(Event::StatusUpdated) => status_updated = true,
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(_) => {
                    self.lb_rx = None;
                    break;
                }
            }
        }
        if status_updated {
            if let Some(r) = self.session.ready_mut() {
                r.refresh_status();
            }
        }
    }

    fn process_keys(&mut self, ctx: &egui::Context) {
        const CMD: egui::Modifiers = egui::Modifiers::COMMAND;

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some(modal) = &self.modal {
                let action = match modal {
                    // Account subpanels (incl. upgrade) stay in Settings — Esc backs the panel.
                    Modal::Settings { .. } => match &self.account_panel {
                        AccountPanel::Upgrade {
                            stage: action::UpgradeStage::Paying,
                            done: None,
                            ..
                        } => None, // mid-charge
                        AccountPanel::Upgrade { .. } => Some(A::UpgradeBack),
                        AccountPanel::Closed => Some(A::CloseModal),
                        _ => Some(A::HideAccountKey),
                    },
                    _ => Some(A::CloseModal),
                };
                if let Some(a) = action {
                    self.queue.push(a);
                }
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            }
        }

        // Onboard welcome: ⌘N Create account · ⌘I Import account.
        if matches!(
            &self.modal,
            Some(Modal::Onboard { mode: action::OnboardMode::Choice, busy: false, .. })
        ) {
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::N)) {
                self.queue
                    .push(A::OnboardSetMode(action::OnboardMode::Create));
            }
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::I)) {
                self.queue
                    .push(A::OnboardSetMode(action::OnboardMode::Import));
            }
        }

        if matches!(self.session, Session::Ready(_)) {
            // Help / Settings toggle even while open (same shortcut again closes).
            // Nested sheets over Settings (QR, upgrade, …) leave these alone.
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::Slash)) {
                match &self.modal {
                    None => self.queue.push(A::OpenHelp),
                    Some(Modal::Help) => self.queue.push(A::CloseModal),
                    Some(_) => {}
                }
            }
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::Comma)) {
                match &self.modal {
                    None => self.queue.push(A::OpenSettings),
                    Some(Modal::Settings { .. }) => self.queue.push(A::CloseModal),
                    Some(_) => {}
                }
            }
        }

        if self.modal.is_none() && matches!(self.session, Session::Ready(_)) {
            // Global chrome shortcuts — safe while editing.
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::N)) {
                self.queue.push(A::Create);
            }
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::E)) {
                self.queue.push(A::ToggleSidebar);
            }
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::S)) {
                self.queue.push(A::SaveAll);
            }
            if ctx.input_mut(|i| i.consume_key(CMD, egui::Key::O)) {
                self.queue.push(A::OpenSearch);
            }
            // No Files-tree keybinding surface: the editor holds focus whenever a
            // doc is open, so ↑↓/⌘C/… almost never hit the tree. Rename / cut /
            // copy / paste / delete stay on context menus (and global chrome above).
        }

        if let Some(modal) = &self.modal {
            // Share: ⏎ stages the field token; ⌘⏎ / Ctrl+⏎ commits Share.
            // Other sheets: either modifier form commits the primary action.
            if matches!(modal, Modal::Share { .. }) {
                let cmd_enter = ctx.input_mut(|i| i.consume_key(CMD, egui::Key::Enter));
                let plain_enter =
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                if cmd_enter {
                    self.queue.push(A::ShareInvite);
                } else if plain_enter {
                    self.queue.push(A::ShareStageField);
                }
            } else {
                // Pay (upgrade confirm): ⌘⏎ only. Other primaries: ⏎ or ⌘⏎.
                let cmd_enter = ctx.input_mut(|i| i.consume_key(CMD, egui::Key::Enter));
                let plain_enter =
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                let pay_commit = matches!(
                    (&modal, &self.account_panel),
                    (
                        Modal::Settings { .. },
                        AccountPanel::Upgrade { stage: action::UpgradeStage::Confirm, .. }
                    )
                );
                let commit = if pay_commit { cmd_enter } else { cmd_enter || plain_enter };
                if commit {
                    match modal {
                        Modal::Delete { .. } => self.queue.push(A::ConfirmDelete),
                        Modal::Create { .. } => self.queue.push(A::ConfirmCreate),
                        Modal::Move { .. } => self.queue.push(A::ConfirmMove),
                        Modal::Rename { id, name, ext } => {
                            // Match sheet primary_enabled — do not dismiss on Enter
                            // when live validation says the name cannot commit.
                            let live = apply::rename_live_status(self, *id, name, ext.as_deref());
                            if live.can_commit {
                                self.queue.push(A::ConfirmRename);
                            }
                        }
                        Modal::AcceptShare { .. } => self.queue.push(A::ConfirmAcceptShare),
                        Modal::DeclineShare { id, .. } => {
                            self.queue.push(A::ConfirmDeclineShare(*id));
                        }
                        Modal::Settings { .. } => match &self.account_panel {
                            AccountPanel::Logout { .. } => self.queue.push(A::ConfirmLogout),
                            AccountPanel::DeleteAccount { .. } => {
                                self.queue.push(A::ConfirmDeleteAccount)
                            }
                            AccountPanel::CancelSub => self.queue.push(A::ConfirmCancelSub),
                            AccountPanel::Upgrade {
                                stage: action::UpgradeStage::EnterCard,
                                ..
                            } => self.queue.push(A::UpgradeNext),
                            AccountPanel::Upgrade {
                                stage: action::UpgradeStage::Confirm, ..
                            } => self.queue.push(A::UpgradeConfirmPay),
                            AccountPanel::Upgrade {
                                stage: action::UpgradeStage::Paying,
                                done: Some(Ok(())),
                                ..
                            } => self.queue.push(A::UpgradeDone),
                            AccountPanel::Upgrade {
                                stage: action::UpgradeStage::Paying,
                                done: Some(Err(_)),
                                ..
                            } => self.queue.push(A::UpgradeBack),
                            _ => {}
                        },
                        Modal::ImportParent { .. } => self.queue.push(A::ConfirmImportParent),
                        Modal::Onboard { .. } => {
                            self.queue.push(A::OnboardSubmit { show_error: true })
                        }
                        Modal::Share { .. } => unreachable!(),
                        _ => {}
                    }
                }
            }
        }
    }

    fn process_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let paths: Vec<_> = dropped.into_iter().filter_map(|f| f.path).collect();
        if paths.is_empty() {
            return;
        }
        // Pick parent when drop lands (folder picker), unless a folder is cursored.
        let parent = self.session.ready().and_then(|r| {
            let id = r.cursor?;
            let files = r.workspace.files.read().unwrap();
            let f = files.get_by_id(id)?;
            if f.is_folder() { Some(f.id) } else { None }
        });
        if let Some(parent) = parent {
            self.queue.push(A::ImportPaths { paths, parent });
        } else {
            self.queue.push(A::OpenImportParent { paths });
        }
    }
}

/// Expand ancestors so `id` is reachable in the Files tree.
pub(crate) fn reveal_in_tree(r: &mut ReadyState, id: Uuid) {
    let files = r.workspace.files.read().unwrap();
    let mut cur = files.get_by_id(id).map(|f| f.parent);
    while let Some(p) = cur {
        r.expanded.insert(p);
        cur = files
            .get_by_id(p)
            .and_then(|f| if f.parent == f.id { None } else { Some(f.parent) });
    }
}

/// Expand ancestors and animate the Files tree to center `id` (tab / open / restore).
pub(crate) fn reveal_and_scroll(r: &mut ReadyState, id: Uuid) {
    reveal_in_tree(r, id);
    r.request_tree_scroll(id);
}

/// Boot / sign-in wait: same elevated plate language as onboard sheets.
fn boot_screen(ctx: &egui::Context, t: &Theme, msg: &str, spinner: bool) {
    // Canvas under the plate (onboard sits on dimmed product chrome; here the
    // whole window is the canvas until we have a session).
    CentralPanel::default()
        .frame(Frame::new().fill(t.neutral_bg_secondary()))
        .show(ctx, |_| {});

    Area::new(Id::new("shell_boot_plate"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Match onboard form width so the plate doesn’t jump at sign-in.
            sheet_panel_fit(ui, t, 380.0, |ui| {
                ui.label(
                    TypeRole::Title
                        .rich("Lockbook")
                        .strong()
                        .color(t.neutral_fg()),
                );
                ui.add(Spacer::new(Space::Md));
                if spinner {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add(Spacer::new(Space::Sm));
                        ui.label(TypeRole::Body.rich(msg).color(t.neutral_fg_secondary()));
                    });
                } else {
                    ui.label(TypeRole::Body.rich(msg).color(t.neutral_fg_secondary()));
                }
            });
        });
}

/// macOS NSWindow chrome helpers (titlebar drag vs interactive tabs).
#[cfg(target_os = "macos")]
pub mod macos_window;
