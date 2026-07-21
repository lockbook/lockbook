#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod theme;
mod util;
mod widgets;

pub use crate::settings::Settings;

// The action surface, exposed for headless observation and scripting — the
// programmatic projection of the same widgets the GUI drives.
pub use crate::widgets::file_tree;
pub use lb::Uuid;

#[cfg(feature = "egui_wgpu_renderer")]
pub use lb_wgpu::*;

use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::thread;

use lb::blocking::Lb;
use lb::model::core_config::Config;
use lb::model::file::File;
use lb::model::file_metadata::FileType;
use lb::model::filename::NameComponents;
use lb::service::events::Event;
use lb::service::import_export::ImportStatus;
use lb::subscribers::status::Status;
use rfd::FileDialog;
use tokio::sync::broadcast::error::TryRecvError;
use workspace_rs::file_cache::{FileCache, FilesExt};
use workspace_rs::show::InputStateExt;
use workspace_rs::theme::palette_v2::Mode;
use workspace_rs::theme::visuals;
use workspace_rs::workspace::Workspace;

use crate::settings::ThemeMode;
use crate::theme::icons;
use crate::theme::tokens::Tokens;
use crate::widgets::file_tree::FileTree;
use crate::widgets::modals::{self, DeleteModal, FolderPickerPurpose, MoveModal, ShareModal};
use crate::widgets::nav;
use crate::widgets::sidebar_views::{self, SharedUi, SidebarPane};
use crate::widgets::settings_panel::{self, SettingsModal, SettingsOutcome};
use crate::widgets::sync_footer::{AccountInfo, SyncFooter};

/// The sidebar extends to the top of the window; a toolbar cluster (Files /
/// Recents / Shared view toggles, then settings) floats at its top-left and
/// stays visible even when the sidebar is closed (Zed-style). `HEADER_CENTER`
/// is the y-center that row aligns to — the center of the native macOS traffic
/// lights (dev), measured at ~16pt from the window top. On macOS the lights sit
/// top-left, so the cluster is pushed clear of them (`TOGGLE_X`); elsewhere a
/// normal inset.
const HEADER_CENTER: f32 = 16.0;
#[cfg(target_os = "macos")]
const TOGGLE_X: f32 = 76.0;
#[cfg(not(target_os = "macos"))]
const TOGGLE_X: f32 = 10.0;

pub struct Lockbook {
    mode: Mode,
    /// Last known OS light/dark from the host (`set_dark_mode` / launch).
    /// Used when `settings.theme_mode == System` — not the same as `mode`
    /// after the user has forced Dark or Light.
    os_dark: bool,
    tree: FileTree,
    files: Vec<File>,
    /// Whether the left sidebar column is visible.
    sidebar_open: bool,
    /// Which body is showing when open (Files / Recents / Shared).
    sidebar_pane: SidebarPane,
    /// Shared-with-me expand + reject-confirm UI state.
    shared_ui: SharedUi,
    session: Session,
    /// Sidebar sync footer chrome (stable message + spin).
    sync_footer: SyncFooter,
    /// Share sheet (context menu → Share).
    share_modal: Option<ShareModal>,
    /// Move-to folder picker (context menu → Move).
    move_modal: Option<MoveModal>,
    /// Delete confirmation (context menu / Delete key).
    delete_modal: Option<DeleteModal>,
    /// Settings panel (gear / ⌘,).
    settings_modal: Option<SettingsModal>,
    /// Persisted prefs (editor toggles, etc.).
    settings: Settings,
}

/// The account lifecycle. `Demo` is the headless/observe default and the state
/// before `start_core` runs; `Loading` awaits the off-thread `lb` init; `Ready`
/// holds the live workspace and file cache once signed in. `SignedOut` is a
/// blank canvas until onboarding exists.
enum Session {
    Demo,
    Loading(Receiver<CoreLoad>),
    SignedOut,
    // Boxed: the live `Workspace` dwarfs the other variants.
    Ready(Box<Ready>),
}

struct Ready {
    /// Shared with the `Workspace` so the tree and editor see one file cache.
    file_cache: Arc<RwLock<FileCache>>,
    workspace: Workspace,
    /// Local pin set — loaded from `lb.list_pinned`, mutated on pin/unpin.
    pinned: std::collections::HashSet<Uuid>,
    /// Snapshot of `lb.status()` — refreshed on `Event::StatusUpdated`.
    status: Status,
    /// Live event stream for status (and future shell reactions).
    events: tokio::sync::broadcast::Receiver<Event>,
    /// Account standing from `get_subscription_info` (None until the
    /// background fetch lands).
    account: Option<AccountInfo>,
    /// Background `get_subscription_info` result; drained once in `update`.
    account_rx: Option<std::sync::mpsc::Receiver<AccountInfo>>,
}

/// Handoff from the core-loading thread (boxed — `Lb`/`FileCache` dwarf the
/// error variant). `file_cache` is `Some` only when signed in (building it, like
/// the workspace, requires an account).
enum CoreLoad {
    Ready(Box<CoreReady>),
    Failed(String),
}

struct CoreReady {
    core: Lb,
    file_cache: Option<FileCache>,
}

/// Blocking `lb` init + file-cache load, run on a worker thread by `start_core`.
fn load_core() -> CoreLoad {
    let core = match Lb::init(Config::ui_config("egui")) {
        Ok(core) => core,
        Err(e) => return CoreLoad::Failed(format!("{e:?}")),
    };
    let file_cache = core
        .get_account()
        .is_ok()
        .then(|| FileCache::new(&core).ok())
        .flatten();
    CoreLoad::Ready(Box::new(CoreReady { core, file_cache }))
}

/// The shell's action vocabulary — composed from its feature widgets' escapes
/// plus the shell's own chrome. Every state change lands here and drains through
/// `apply`, the single point that touches app state.
enum Action {
    Tree(file_tree::Op),
    Pinned(widgets::pinned::Op),
    SidebarView(sidebar_views::Op),
    /// Zed-style view toggle: open that pane, or close the sidebar if it is
    /// already the active pane.
    SelectSidebarPane(SidebarPane),
    OpenSettings,
    NewNote,
    Import,
    OpenSearch,
    RequestSync,
}

impl From<file_tree::Op> for Action {
    fn from(op: file_tree::Op) -> Self {
        Action::Tree(op)
    }
}

#[derive(Debug, Default)]
pub struct Response {
    pub close: bool,
}

impl Lockbook {
    pub fn new(ctx: &egui::Context) -> Self {
        // Host typically installed OS appearance just before `new`.
        let os_dark = ctx.style().visuals.dark_mode;
        let mode = if os_dark { Mode::Dark } else { Mode::Light };
        Lockbook {
            mode,
            os_dark,
            tree: FileTree::default(),
            files: file_tree::demo_files(),
            sidebar_open: true,
            sidebar_pane: SidebarPane::Files,
            shared_ui: SharedUi::default(),
            session: Session::Demo,
            sync_footer: SyncFooter::default(),
            share_modal: None,
            move_modal: None,
            delete_modal: None,
            settings_modal: None,
            settings: Settings::read_from_file().unwrap_or_default(),
        }
    }

    /// Kick off the `lb` core load on a worker thread (init is blocking). The
    /// host calls this once; the headless harness never does, so it stays in
    /// `Demo`. `update` polls the result and transitions to `Ready`/`SignedOut`.
    pub fn start_core(&mut self, ctx: &egui::Context) {
        if !matches!(self.session, Session::Demo) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(load_core());
            ctx.request_repaint();
        });
        self.session = Session::Loading(rx);
    }

    /// Deferred one-time setup: fonts, image loaders, and the color theme. egui
    /// resets visuals between init and the first frame, so this runs on frame one.
    pub fn deferred_init(&mut self, ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        workspace_rs::register_fonts(&mut fonts);
        theme::icons::register(&mut fonts);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(ctx);

        // Prefer the mode the host already installed (system appearance at launch),
        // then resolve against Settings (System / Dark / Light + color theme).
        self.os_dark = ctx.style().visuals.dark_mode;
        let mode = crate::theme::resolve_mode(self.settings.theme_mode, self.os_dark);
        self.mode = mode;
        crate::theme::apply_settings(&self.settings, ctx, self.os_dark);
        visuals::init(ctx);
    }

    /// Apply light/dark from the OS. Always records `os_dark` so choosing
    /// System later is correct; only re-themes when preference is System.
    pub fn set_mode(&mut self, ctx: &egui::Context, mode: Mode) {
        self.os_dark = mode == Mode::Dark;
        if self.settings.theme_mode != ThemeMode::System {
            return;
        }
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        crate::theme::apply_settings(&self.settings, ctx, self.os_dark);
    }

    /// Convenience for hosts that only have a boolean dark flag (winit / dark-light).
    pub fn set_dark_mode(&mut self, ctx: &egui::Context, dark: bool) {
        self.set_mode(ctx, if dark { Mode::Dark } else { Mode::Light });
    }

    pub fn is_dev(&self) -> bool {
        false
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Response {
        let t = theme::tokens::Tokens::new(ctx);
        let mut actions: Vec<Action> = Vec::new();

        // Core-load handoff: when the worker thread delivers, build the live
        // workspace (once, on this thread — it needs the render `ctx`).
        let loaded = match &self.session {
            Session::Loading(rx) => rx.try_recv().ok(),
            _ => None,
        };
        if let Some(load) = loaded {
            self.session = match load {
                CoreLoad::Ready(cr) => {
                    let CoreReady { core, file_cache } = *cr;
                    match file_cache {
                        Some(fc) => {
                            let file_cache = Arc::new(RwLock::new(fc));
                            let mut workspace =
                                Workspace::new(&core, ctx, true, true, Some(file_cache.clone()));
                            // Tab strip defaults to CHROME_STRIP_H (matches toolbar).
                            workspace
                                .cfg
                                .set_contact_linked_sites(self.settings.contact_linked_sites);
                            let pinned = core
                                .list_pinned()
                                .unwrap_or_default()
                                .into_iter()
                                .collect();
                            let status = core.status();
                            let events = core.subscribe();
                            // Account standing is network-bound — don't block
                            // first paint. Footer hover omits Account until set.
                            let (account_tx, account_rx) = std::sync::mpsc::channel();
                            let core_account = core.clone();
                            let ctx_account = ctx.clone();
                            thread::spawn(move || {
                                let cap = core_account
                                    .get_usage()
                                    .ok()
                                    .map(|u| u.data_cap.exact);
                                let account = match core_account.get_subscription_info() {
                                    Ok(info) => {
                                        AccountInfo::from_subscription_and_cap(info, cap)
                                    }
                                    Err(e) => {
                                        log::warn!("get_subscription_info failed: {e:?}");
                                        return;
                                    }
                                };
                                let _ = account_tx.send(account);
                                ctx_account.request_repaint();
                            });
                            Session::Ready(Box::new(Ready {
                                file_cache,
                                workspace,
                                pinned,
                                status,
                                events,
                                account: None,
                                account_rx: Some(account_rx),
                            }))
                        }
                        None => Session::SignedOut,
                    }
                }
                CoreLoad::Failed(e) => {
                    log::error!("lb core init failed: {e}");
                    Session::SignedOut
                }
            };
        }

        // Drain lb events — status updates drive the sync footer.
        if let Session::Ready(r) = &mut self.session {
            // Background account standing fetch (once on Ready).
            if let Some(rx) = r.account_rx.take() {
                match rx.try_recv() {
                    Ok(account) => r.account = Some(account),
                    Err(std::sync::mpsc::TryRecvError::Empty) => r.account_rx = Some(rx),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
                }
            }
            loop {
                match r.events.try_recv() {
                    Ok(Event::StatusUpdated) => {
                        r.status = r.workspace.core.status();
                        ctx.request_repaint();
                    }
                    Ok(_) => {}
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Lagged(_)) => {
                        r.status = r.workspace.core.status();
                        ctx.request_repaint();
                        break;
                    }
                    Err(TryRecvError::Closed) => break,
                }
            }
        }

        // egui draws the sidebar resize handle with the "highly visible"
        // hovered/active foreground strokes; soften both to a faint hairline.
        // Also shrink the side resize grab (default 5px each side of the edge)
        // so it barely overlaps the tree scrollbar, which we inset from the
        // panel edge — macOS-like separation of scroll vs resize.
        let resize_stroke = egui::Stroke::new(1.0, t.line());
        let saved = {
            let s = ctx.style();
            (
                s.visuals.widgets.hovered.fg_stroke,
                s.visuals.widgets.active.fg_stroke,
                s.interaction.resize_grab_radius_side,
            )
        };
        ctx.style_mut(|s| {
            s.visuals.widgets.hovered.fg_stroke = resize_stroke;
            s.visuals.widgets.active.fg_stroke = resize_stroke;
            // ~3px into the sidebar + ~3px into the content (was 5+5).
            s.interaction.resize_grab_radius_side = 3.0;
        });
        // Latch separator-drag *before* the tree paints so a leftward drag
        // (pointer over the tree while width lags) keeps the floating bar dormant.
        sync_sidebar_resizing_latch(ctx);

        // Shell modals: hold focus + consume input *before* workspace so the
        // markdown editor (and tab shortcuts) can't type or act under the sheet.
        if self.settings_modal.is_some() {
            // Search field paints later this frame; claim focus so the editor
            // doesn't reclaim when `focused().is_none()`. Esc still closes.
            let search_id = egui::Id::new("settings_search").with("edit");
            ctx.memory_mut(|m| {
                if m.focused().is_none() {
                    m.request_focus(search_id);
                }
            });
        } else if self.delete_modal.is_some() {
            widgets::modals::hold_modal_focus(ctx, widgets::modals::ModalKind::Delete);
        } else if self.share_modal.is_some() {
            widgets::modals::hold_modal_focus(ctx, widgets::modals::ModalKind::Share);
        } else if self.move_modal.is_some() {
            widgets::modals::hold_modal_focus(ctx, widgets::modals::ModalKind::Move);
        }

        // ⌘, toggles settings (VS Code / Raycast style). When another modal is
        // open, leave the shortcut alone so sheets keep their own key handling.
        if self.share_modal.is_none()
            && self.move_modal.is_none()
            && self.delete_modal.is_none()
            && ctx.input_mut(|i| {
                i.consume_key_exact(egui::Modifiers::COMMAND, egui::Key::Comma)
            })
        {
            if self.settings_modal.is_some() {
                self.settings_modal = None;
                ctx.request_repaint();
            } else {
                actions.push(Action::OpenSettings);
            }
        }

        // Share / move / delete / settings keyboard **before** workspace.
        let mut settings_close = false;
        if let Some(modal) = self.settings_modal.as_mut() {
            settings_close = settings_panel::handle_settings_keyboard(ctx, modal);
        }
        let mut share_key = widgets::modals::ShareKeyResult::None;
        if let Some(modal) = self.share_modal.as_mut() {
            if let Session::Ready(r) = &self.session {
                let files = r.file_cache.read().unwrap();
                share_key = widgets::modals::handle_share_keyboard(ctx, &*files, modal);
            }
        }
        let mut move_key = widgets::modals::MoveKeyResult::None;
        if let Some(modal) = self.move_modal.as_mut() {
            if let Session::Ready(r) = &self.session {
                let files = r.file_cache.read().unwrap();
                move_key = widgets::modals::handle_move_keyboard(ctx, &*files, modal);
            }
        }
        let mut delete_key = widgets::modals::DeleteKeyResult::None;
        if self.delete_modal.is_some() {
            delete_key = widgets::modals::handle_delete_keyboard(ctx);
        }
        if settings_close {
            self.settings_modal = None;
            ctx.request_repaint();
        }

        if self.sidebar_open {
            self.sidebar(ctx, &t, &mut actions);
        }
        // SidePanel registers its resize interact after contents — pick that up
        // for next frame's latch (and for anything painted after the sidebar).
        sync_sidebar_resizing_latch(ctx);
        ctx.style_mut(|s| {
            s.visuals.widgets.hovered.fg_stroke = saved.0;
            s.visuals.widgets.active.fg_stroke = saved.1;
            s.interaction.resize_grab_radius_side = saved.2;
        });

        // Windows/Linux: the window is borderless, so a drag strip across the
        // top stands in for the native title bar — drag to move, double-click to
        // toggle maximize. The toolbar cluster layers above it (Foreground) so
        // its buttons still take clicks.
        #[cfg(not(target_os = "macos"))]
        egui::Area::new("window_drag".into())
            .fixed_pos(ctx.screen_rect().min)
            .show(ctx, |ui| {
                let size = egui::vec2(ctx.screen_rect().width(), HEADER_CENTER * 2.0);
                let resp = ui.allocate_response(size, egui::Sense::click_and_drag());
                if resp.drag_started_by(egui::PointerButton::Primary) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if resp.double_clicked_by(egui::PointerButton::Primary) {
                    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }
            });

        #[cfg(not(target_os = "macos"))]
        window_resize_edges(ctx);

        // Zed-style sidebar view toggles (Files / Recents / Shared) + settings.
        // Always visible — not a single hide control. Clicking the active view
        // dismisses the sidebar; clicking another opens that pane.
        // Foreground order keeps clicks above the title-bar drag strip.
        egui::Area::new("sidebar_toolbar".into())
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(TOGGLE_X, HEADER_CENTER - nav::ICON_BUTTON_SIZE / 2.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for pane in SidebarPane::ALL {
                        let active = self.sidebar_open && self.sidebar_pane == pane;
                        let resp = nav::icon_button_active(ui, &t, pane.icon(), active);
                        workspace_rs::widgets::tip_text(ui.ctx(), &resp, pane.title());
                        if resp.clicked() {
                            actions.push(Action::SelectSidebarPane(pane));
                        }
                    }
                    let gear = nav::icon_button(ui, &t, icons::GEAR);
                    workspace_rs::widgets::tip_text(ui.ctx(), &gear, "Settings");
                    if gear.clicked() {
                        if self.settings_modal.is_some() {
                            self.settings_modal = None;
                            ctx.request_repaint();
                        } else {
                            actions.push(Action::OpenSettings);
                        }
                    }
                });
            });

        // Windows/Linux: minimize / maximize / close, anchored top-right. Ghost
        // buttons like the toolbar cluster; close firms red. Foreground so they
        // layer above the drag strip beneath.
        #[cfg(not(target_os = "macos"))]
        egui::Area::new("window_controls".into())
            .order(egui::Order::Foreground)
            .anchor(
                egui::Align2::RIGHT_TOP,
                egui::vec2(-6.0, HEADER_CENTER - nav::ICON_BUTTON_SIZE / 2.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if nav::window_button(ui, &t, icons::MINUS, false).clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    let max_icon = if maximized { icons::COPY } else { icons::SQUARE };
                    if nav::window_button(ui, &t, max_icon, false).clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if nav::window_button(ui, &t, icons::X, true).clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

        // Center: live workspace when signed in; empty canvas while loading /
        // signed out (onboarding later). Drain workspace outputs so shell
        // chrome and the editor stay in lockstep (create → open, tab → tree).
        if let Session::Ready(r) = &mut self.session {
            let mut created = None;
            let mut selected_file = None;
            let mut cache_updated = false;
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(t.canvas()))
                .show(ctx, |ui| {
                    let out = r.workspace.show(ui);
                    created = out.file_created;
                    selected_file = out.selected_file;
                    cache_updated = out.file_cache_updated;
                });
            if cache_updated {
                let files = r.file_cache.read().unwrap();
                self.tree.prune_missing(&*files);
            }
            if let Some(Ok(f)) = created {
                if f.is_document() {
                    r.workspace.open_file(f.id, true, true);
                    // open_file stamps selected_file next frame; reveal now too.
                    selected_file = Some(f.id);
                }
            }
            if let Some(id) = selected_file {
                self.reveal_in_tree(id);
            }
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(t.canvas()))
                .show(ctx, |_ui| {});
        }

        // SidePanel's resize grab extends equally into the content, but CentralPanel
        // / workspace are shown after and cover that half. Reclaim it on top.
        if self.sidebar_open {
            sidebar_resize_over_workspace(ctx, &t);
        }

        for action in actions {
            self.apply(ctx, action);
        }
        // Tree cursor drives create/import parent (master focused_parent sync).
        self.sync_focused_parent_from_tree();
        // Share / Move sheets (opened from context menus).
        self.show_modals(ctx, &t);
        // Apply keyboard confirm / dismiss after paint so the sheet can close cleanly.
        match share_key {
            widgets::modals::ShareKeyResult::None => {}
            widgets::modals::ShareKeyResult::Dismiss => {
                self.share_modal = None;
            }
            widgets::modals::ShareKeyResult::Submit => {
                if let Some(modal) = self.share_modal.as_ref() {
                    let pending = modal.pending.clone();
                    if !pending.is_empty() {
                        self.submit_share_batch(pending);
                    }
                }
            }
        }
        match move_key {
            widgets::modals::MoveKeyResult::None => {}
            widgets::modals::MoveKeyResult::Dismiss => {
                self.move_modal = None;
            }
            widgets::modals::MoveKeyResult::Confirm(parent) => {
                if let Some(modal) = self.move_modal.take() {
                    self.confirm_folder_picker(modal.purpose, parent);
                }
            }
        }
        match delete_key {
            widgets::modals::DeleteKeyResult::None => {}
            widgets::modals::DeleteKeyResult::Dismiss => {
                self.delete_modal = None;
            }
            widgets::modals::DeleteKeyResult::Confirm => {
                if let Some(modal) = self.delete_modal.take() {
                    self.confirm_delete(modal.ids);
                }
            }
        }
        Response { close: ctx.input(|i| i.viewport().close_requested()) }
    }

    /// Draw open shell modals (settings / share / move / delete) above all chrome.
    fn show_modals(&mut self, ctx: &egui::Context, t: &Tokens) {
        if self.settings_modal.is_none()
            && self.share_modal.is_none()
            && self.move_modal.is_none()
            && self.delete_modal.is_none()
        {
            return;
        }
        if !matches!(self.session, Session::Ready(_)) {
            self.settings_modal = None;
            self.share_modal = None;
            self.move_modal = None;
            self.delete_modal = None;
            return;
        }

        // Outcomes collected outside the paint closure so we can mutably use self.
        let mut close_share = false;
        let mut submit_share: Option<Vec<modals::PendingShare>> = None;
        let mut close_move = false;
        let mut folder_pick: Option<(FolderPickerPurpose, Uuid)> = None;
        let mut close_delete = false;
        let mut confirm_delete = false;
        let mut close_settings = false;
        let mut settings_logout: Option<String> = None;
        let mut settings_delete_account = false;
        let mut settings_cancel_sub = false;
        let mut settings_save_prefs: Option<bool> = None;
        let mut settings_appearance: Option<(ThemeMode, String)> = None;
        let mut settings_plan: Option<AccountInfo> = None;

        {
            let Session::Ready(r) = &self.session else {
                return;
            };
            let files = r.file_cache.read().unwrap();
            let core = r.workspace.core.clone();

            if let Some(modal) = self.settings_modal.as_mut() {
                match settings_panel::show_settings(ctx, t, modal, &core) {
                    SettingsOutcome::Open => {}
                    SettingsOutcome::Closed => close_settings = true,
                    SettingsOutcome::Logout { writeable_path } => {
                        settings_logout = Some(writeable_path);
                    }
                    SettingsOutcome::DeleteAccount => settings_delete_account = true,
                    SettingsOutcome::CancelSubscription => settings_cancel_sub = true,
                    SettingsOutcome::SavePrefs {
                        contact_linked_sites,
                    } => {
                        settings_save_prefs = Some(contact_linked_sites);
                    }
                    SettingsOutcome::SaveAppearance {
                        theme_mode,
                        theme_name,
                    } => {
                        settings_appearance = Some((theme_mode, theme_name));
                    }
                    SettingsOutcome::PlanRefreshed(plan) => {
                        settings_plan = Some(plan);
                    }
                }
            }

            if let Some(modal) = self.share_modal.as_mut() {
                // Debounced get_public_key for free-typed “new person” hits.
                modal.maintain_lookup(&core, ctx, &*files);
                if modals::show_modal_dim(
                    ctx,
                    egui::Id::new("lb_modal_dim_share"),
                    modals::share_sheet_layer_id(),
                ) {
                    close_share = true;
                }
                match modals::show_share(ctx, t, &*files, modal) {
                    modals::ShareOutcome::Open => {}
                    modals::ShareOutcome::Closed => close_share = true,
                    modals::ShareOutcome::Submit { pending } => {
                        submit_share = Some(pending);
                    }
                }
            }

            if let Some(modal) = self.move_modal.as_mut() {
                let pinned = r.pinned.clone();
                // Dim + outside click live inside `show_move` (same helper).
                match modals::show_move(ctx, t, &*files, modal, &pinned) {
                    modals::MoveOutcome::Open => {}
                    modals::MoveOutcome::Closed => close_move = true,
                    modals::MoveOutcome::Confirm { parent } => {
                        folder_pick = Some((modal.purpose.clone(), parent));
                    }
                }
            }

            if let Some(modal) = self.delete_modal.as_ref() {
                if modals::show_modal_dim(
                    ctx,
                    egui::Id::new("lb_modal_dim_delete"),
                    modals::delete_sheet_layer_id(),
                ) {
                    close_delete = true;
                }
                match modals::show_delete(ctx, t, modal) {
                    modals::DeleteOutcome::Open => {}
                    modals::DeleteOutcome::Closed => close_delete = true,
                    modals::DeleteOutcome::Confirm => confirm_delete = true,
                }
            }
        }

        if close_settings {
            self.settings_modal = None;
            ctx.request_repaint();
        }
        if let Some(path) = settings_logout {
            self.settings_modal = None;
            let _ = std::fs::remove_dir_all(&path);
            log::info!("logged out; removed {path}");
            std::process::exit(0);
        }
        if settings_delete_account {
            self.settings_modal = None;
            if let Session::Ready(r) = &self.session {
                if let Err(e) = r.workspace.core.delete_account() {
                    log::error!("delete account failed: {e:?}");
                } else {
                    let path = r.workspace.core.get_config().writeable_path.clone();
                    let _ = std::fs::remove_dir_all(&path);
                    std::process::exit(0);
                }
            }
        }
        if settings_cancel_sub {
            if let Session::Ready(r) = &self.session {
                match r.workspace.core.cancel_subscription() {
                    Ok(()) => {
                        let usage = r.workspace.core.get_usage().ok();
                        let cap = usage.as_ref().map(|u| u.data_cap.exact);
                        let plan = r
                            .workspace
                            .core
                            .get_subscription_info()
                            .ok()
                            .map(|info| AccountInfo::from_subscription_and_cap(info, cap))
                            .unwrap_or(AccountInfo {
                                tier: crate::widgets::sync_footer::AccountTier::Free,
                                detail: Some("Canceled".into()),
                                source: None,
                            });
                        if let Some(m) = self.settings_modal.as_mut() {
                            m.error = None;
                            m.data.plan = Some(plan.clone());
                            if let Some(u) = usage {
                                m.data.usage = Some(u);
                            }
                            m.set_flash("Subscription canceled. You’re on Free.");
                        }
                        if let Session::Ready(r) = &mut self.session {
                            r.account = Some(plan);
                        }
                    }
                    Err(e) => {
                        // Already free (e.g. canceled Stripe still shows a platform
                        // row) — heal UI instead of a confusing error.
                        let msg = format!("{e}");
                        let already_free = msg.to_lowercase().contains("not premium")
                            || msg.to_lowercase().contains("notpremium");
                        if already_free {
                            let usage = r.workspace.core.get_usage().ok();
                            let cap = usage.as_ref().map(|u| u.data_cap.exact);
                            let plan = r
                                .workspace
                                .core
                                .get_subscription_info()
                                .ok()
                                .map(|info| AccountInfo::from_subscription_and_cap(info, cap))
                                .unwrap_or(AccountInfo {
                                    tier: crate::widgets::sync_footer::AccountTier::Free,
                                    detail: Some("Canceled".into()),
                                    source: None,
                                });
                            if let Some(m) = self.settings_modal.as_mut() {
                                m.error = None;
                                m.data.plan = Some(plan.clone());
                                if let Some(u) = usage {
                                    m.data.usage = Some(u);
                                }
                                m.set_flash("You’re already on Free.");
                            }
                            if let Session::Ready(r) = &mut self.session {
                                r.account = Some(plan);
                            }
                        } else if let Some(m) = self.settings_modal.as_mut() {
                            m.error = Some(format!("Couldn’t cancel: {e}"));
                        }
                    }
                }
            }
        }
        if let Some(contact) = settings_save_prefs {
            self.settings.contact_linked_sites = contact;
            if let Err(e) = self.settings.write_to_file() {
                log::warn!("failed to write settings: {e}");
            }
            if let Session::Ready(r) = &mut self.session {
                r.workspace.cfg.set_contact_linked_sites(contact);
            }
        }
        if let Some((theme_mode, theme_name)) = settings_appearance {
            self.settings.theme_mode = theme_mode;
            self.settings.theme_name = theme_name;
            if let Err(e) = self.settings.write_to_file() {
                log::warn!("failed to write settings: {e}");
            }
            // System → last OS appearance from the host. Dark/Light ignore OS.
            // Never use `self.mode` here — that is the *app* theme and stays
            // wrong after a forced Dark/Light when re-selecting System.
            let os_dark = self.os_dark;
            self.mode = crate::theme::resolve_mode(theme_mode, os_dark);
            crate::theme::apply_settings(&self.settings, ctx, os_dark);
        }
        if let Some(plan) = settings_plan {
            if let Session::Ready(r) = &mut self.session {
                r.account = Some(plan);
            }
        }
        if close_share {
            self.share_modal = None;
            ctx.request_repaint();
        }
        if let Some(pending) = submit_share {
            self.submit_share_batch(pending);
        }
        if close_move {
            self.move_modal = None;
            ctx.request_repaint();
        }
        if let Some((purpose, parent)) = folder_pick {
            self.move_modal = None;
            self.confirm_folder_picker(purpose, parent);
            ctx.request_repaint();
        }
        if close_delete {
            self.delete_modal = None;
            ctx.request_repaint();
        }
        if confirm_delete {
            if let Some(modal) = self.delete_modal.take() {
                self.confirm_delete(modal.ids);
            }
        }
    }

    /// Open the delete / remove-from-files confirmation with names from the
    /// live file cache. Organized shares (not owned by us — link targets in the
    /// tree) use remove-from-files copy; own files use true delete.
    fn open_delete_confirm(&mut self, ids: Vec<Uuid>) {
        if ids.is_empty() {
            return;
        }
        let (names, all_organized_shares) = match &self.session {
            Session::Ready(r) => {
                let me = r.workspace.account.username.as_str();
                let files = r.file_cache.read().unwrap();
                let names: Vec<String> = ids
                    .iter()
                    .filter_map(|id| files.get_by_id(*id).map(|f| f.name.clone()))
                    .collect();
                let all_organized_shares = !ids.is_empty()
                    && ids.iter().all(|id| {
                        files
                            .get_by_id(*id)
                            .is_some_and(|f| file_tree::is_organized_share(&*files, f, me))
                    });
                (names, all_organized_shares)
            }
            _ => (Vec::new(), false),
        };
        self.delete_modal = Some(if all_organized_shares {
            DeleteModal::remove_from_files(ids, names)
        } else {
            DeleteModal::new(ids, names)
        });
    }

    fn confirm_delete(&mut self, ids: Vec<Uuid>) {
        let Session::Ready(r) = &mut self.session else {
            return;
        };
        for id in ids {
            r.workspace.delete_file(id);
        }
        let files = r.file_cache.read().unwrap();
        self.tree.prune_missing(&*files);
    }

    /// The sidebar, top to bottom. Fixed top (toolbar strip + optional action
    /// chips) and bottom (sync status) reserve space as nested panels; the
    /// active pane body (Files / Recents / Shared) fills the remainder.
    fn sidebar(&mut self, ctx: &egui::Context, t: &Tokens, actions: &mut Vec<Action>) {
        egui::SidePanel::left("sidebar")
            .resizable(true)
            // Match Apple `navigationSplitViewColumnWidth(min: 268, ideal: 300, max: 500)`
            // so New / Import / Search show labels at the default width.
            .default_width(300.0)
            .width_range(268.0..=500.0)
            .show_separator_line(false)
            .frame(egui::Frame::default().fill(t.surface()))
            .show(ctx, |ui| {
                // Action chips only on Files (New / Import / Search). Other
                // panes get a short top inset so content clears the toolbar.
                let show_chips = self.sidebar_pane == SidebarPane::Files;
                // Files head stays surface (chips off canvas tree). Recents /
                // Shared are full-canvas — contrast is the vertical sidebar edge.
                let head_fill = if show_chips { t.surface() } else { t.canvas() };
                egui::TopBottomPanel::top("sidebar_head")
                    .show_separator_line(false)
                    .frame(
                        egui::Frame::default()
                            .fill(head_fill)
                            .inner_margin(egui::Margin {
                                left: 8,
                                right: 8,
                                top: 0,
                                bottom: if show_chips { 14 } else { 4 },
                            }),
                    )
                    .show_inside(ui, |ui| {
                        if show_chips {
                            sidebar_head(ui, t, actions);
                        } else {
                            // Match the Files toolbar strip height so pane
                            // titles sit below the floating view toggles.
                            ui.add_space(HEADER_CENTER * 2.0);
                        }
                    });

                // Sync footer needs `&mut self.sync_footer` + status; draw bottom
                // panel with a snapshot of status/account (avoid borrow vs. actions).
                let (status_snapshot, account_snapshot) = match &self.session {
                    Session::Ready(r) => (Some(r.status.clone()), r.account.clone()),
                    _ => (None, None),
                };
                egui::TopBottomPanel::bottom("sidebar_foot")
                    .show_separator_line(false)
                    .frame(
                        egui::Frame::default()
                            .fill(t.surface())
                            // Vertical air is owned by SyncFooter (symmetric pads).
                            .inner_margin(egui::Margin::ZERO),
                    )
                    .show_inside(ui, |ui| {
                        if let Some(status) = &status_snapshot {
                            if self
                                .sync_footer
                                .show(ui, t, status, account_snapshot.as_ref())
                            {
                                actions.push(Action::RequestSync);
                            }
                        } else {
                            // Demo / signed-out: quiet idle footer.
                            let idle = Status::default();
                            let _ = self.sync_footer.show(ui, t, &idle, None);
                        }
                    });

                let pane = self.sidebar_pane;
                // All pane bodies are canvas (tree / recents / shared). Files
                // still has a surface pin strip above the tree; Recents/Shared
                // are full canvas — contrast comes from the vertical edge.
                let body_frame = egui::Frame::default().fill(t.canvas());
                egui::CentralPanel::default()
                    .frame(body_frame)
                    .show_inside(ui, |ui| match pane {
                        SidebarPane::Files => self.sidebar_files_body(ui, t, actions),
                        SidebarPane::Recents => self.sidebar_recents_body(ui, t, actions),
                        SidebarPane::Shared => self.sidebar_shared_body(ui, t, actions),
                    });
            });
    }

    /// Recents pane: age-bucketed docs (pin strip lives only on Files).
    fn sidebar_recents_body(
        &mut self, ui: &mut egui::Ui, t: &Tokens, actions: &mut Vec<Action>,
    ) {
        match &self.session {
            Session::Ready(r) => {
                // Clone before locking the file cache (same borrow tree).
                let me = r.workspace.account.username.clone();
                let has_clip = self.tree.has_clip();
                let files = r.file_cache.read().unwrap();
                if let Some(op) = sidebar_views::show_recents(
                    ui,
                    t,
                    &*files,
                    Some(me.as_str()),
                    &r.pinned,
                    has_clip,
                    Some(&r.status),
                ) {
                    actions.push(Action::SidebarView(op));
                }
            }
            _ => {
                let empty_pins = std::collections::HashSet::new();
                let has_clip = self.tree.has_clip();
                if let Some(op) = sidebar_views::show_recents(
                    ui,
                    t,
                    &self.files,
                    None,
                    &empty_pins,
                    has_clip,
                    None,
                ) {
                    actions.push(Action::SidebarView(op));
                }
            }
        }
    }

    /// Shared-with-me: pending shares from the live file cache.
    fn sidebar_shared_body(
        &mut self, ui: &mut egui::Ui, t: &Tokens, actions: &mut Vec<Action>,
    ) {
        let op = match &self.session {
            Session::Ready(r) => {
                let me = r.workspace.account.username.clone();
                let files = r.file_cache.read().unwrap();
                sidebar_views::show_shared(ui, t, &files, me.as_str(), &mut self.shared_ui)
            }
            _ => {
                sidebar_views::show_shared_empty(ui, t);
                None
            }
        };
        if let Some(op) = op {
            actions.push(Action::SidebarView(op));
        }
    }

    /// Files pane: pins on surface, tree on canvas (live cache when signed in).
    fn sidebar_files_body(
        &mut self, ui: &mut egui::Ui, t: &Tokens, actions: &mut Vec<Action>,
    ) {
        let empty_pins = std::collections::HashSet::new();
        match &self.session {
            Session::Ready(r) => {
                let files = r.file_cache.read().unwrap();
                // Surface band for the pin strip (chips are canvas on top of it).
                if let Some(op) = widgets::pinned::show(ui, t, &*files, &r.pinned) {
                    actions.push(Action::Pinned(op));
                }
                let me = r.workspace.account.username.as_str();
                if let Some(op) =
                    self.tree
                        .show(ui, t, &*files, &r.pinned, Some(me), Some(&r.status))
                {
                    actions.push(op.into());
                }
            }
            _ => {
                if let Some(op) = self.tree.show(ui, t, &self.files, &empty_pins, None, None) {
                    actions.push(op.into());
                }
            }
        }
    }

    /// The one place shell state changes. Model mutations go through `Workspace`
    /// when signed in; otherwise they're no-ops (demo / signed-out).
    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Tree(op) => match op {
                file_tree::Op::TogglePin { id } => self.toggle_pin(id),
                file_tree::Op::Share { id } => self.open_share(id),
                file_tree::Op::CopyLink { id } => self.copy_file_link(ctx, id),
                file_tree::Op::Move { ids } => {
                    let root = match &self.session {
                        Session::Ready(r) => Some(r.file_cache.read().unwrap().root().id),
                        _ => None,
                    };
                    self.move_modal = Some(MoveModal::new(ids, root));
                }
                file_tree::Op::MoveInto { ids, parent } => self.move_files(ids, parent),
                file_tree::Op::CopyInto { ids, parent } => self.copy_into(ids, parent),
                file_tree::Op::Export { ids } => self.export_files(ids),
                file_tree::Op::Duplicate { id } => self.duplicate_file(id),
                file_tree::Op::Delete { ids } => self.open_delete_confirm(ids),
                file_tree::Op::Open { id, new_tab } => {
                    let Session::Ready(r) = &mut self.session else {
                        log::info!("open with no workspace");
                        return;
                    };
                    r.workspace.open_file(id, true, new_tab);
                    // Same-frame reveal so the tree doesn't wait a frame for
                    // `selected_file` from workspace.
                    self.reveal_in_tree(id);
                }
                op => {
                    let Session::Ready(r) = &mut self.session else {
                        log::info!("tree op with no workspace: {op:?}");
                        return;
                    };
                    match op {
                        file_tree::Op::Open { .. }
                        | file_tree::Op::TogglePin { .. }
                        | file_tree::Op::Share { .. }
                        | file_tree::Op::CopyLink { .. }
                        | file_tree::Op::Move { .. }
                        | file_tree::Op::MoveInto { .. }
                        | file_tree::Op::CopyInto { .. }
                        | file_tree::Op::Export { .. }
                        | file_tree::Op::Duplicate { .. }
                        | file_tree::Op::Delete { .. } => unreachable!(),
                        file_tree::Op::CreateDoc { parent } => {
                            r.workspace.create_doc_at(false, parent)
                        }
                        file_tree::Op::CreateFolder { parent } => {
                            r.workspace.create_folder_at(parent)
                        }
                        file_tree::Op::Rename { id, name } => {
                            r.workspace.rename_file((id, name), true)
                        }
                    }
                }
            },
            Action::Pinned(op) => match op {
                widgets::pinned::Op::Open { id } => self.open_pinned(id),
                widgets::pinned::Op::Unpin { id } => {
                    // Only unpin if currently pinned (menu already says Unpin).
                    let is_pinned = matches!(
                        &self.session,
                        Session::Ready(r) if r.pinned.contains(&id)
                    );
                    if is_pinned {
                        self.toggle_pin(id);
                    }
                }
            },
            Action::SidebarView(op) => match op {
                sidebar_views::Op::Open { id, new_tab } => {
                    let Session::Ready(r) = &mut self.session else {
                        log::info!("shared/recents open with no workspace");
                        return;
                    };
                    r.workspace.open_file(id, true, new_tab);
                    self.reveal_in_tree(id);
                }
                sidebar_views::Op::AcceptShare { id, name } => {
                    // Same folder-destination picker as Move.
                    let root = match &self.session {
                        Session::Ready(r) => Some(r.file_cache.read().unwrap().root().id),
                        _ => None,
                    };
                    self.move_modal = Some(MoveModal::accept_share(id, name, root));
                }
                sidebar_views::Op::RejectShare { id } => {
                    self.reject_share(ctx, id);
                }
                // Recents context menu → same shell paths as the file tree.
                sidebar_views::Op::Rename { id } => {
                    self.reveal_in_tree(id);
                    let Session::Ready(r) = &self.session else {
                        return;
                    };
                    let files = r.file_cache.read().unwrap();
                    self.tree
                        .apply(file_tree::Action::BeginRename(id), &*files);
                }
                sidebar_views::Op::Share { id } => self.open_share(id),
                sidebar_views::Op::CopyLink { id } => self.copy_file_link(ctx, id),
                sidebar_views::Op::TogglePin { id } => self.toggle_pin(id),
                sidebar_views::Op::Duplicate { id } => self.duplicate_file(id),
                sidebar_views::Op::Export { id } => self.export_files(vec![id]),
                sidebar_views::Op::Move { id } => {
                    let root = match &self.session {
                        Session::Ready(r) => Some(r.file_cache.read().unwrap().root().id),
                        _ => None,
                    };
                    self.move_modal = Some(MoveModal::new(vec![id], root));
                }
                sidebar_views::Op::Cut { id } => {
                    let Session::Ready(r) = &self.session else {
                        return;
                    };
                    let files = r.file_cache.read().unwrap();
                    self.tree.apply(file_tree::Action::Select(id), &*files);
                    self.tree
                        .apply(file_tree::Action::CutSelected, &*files);
                }
                sidebar_views::Op::Copy { id } => {
                    let Session::Ready(r) = &self.session else {
                        return;
                    };
                    let files = r.file_cache.read().unwrap();
                    self.tree.apply(file_tree::Action::Select(id), &*files);
                    self.tree
                        .apply(file_tree::Action::CopySelected, &*files);
                }
                sidebar_views::Op::PasteIntoParent { id } => {
                    let Session::Ready(r) = &self.session else {
                        return;
                    };
                    let files = r.file_cache.read().unwrap();
                    let Some(parent) = files.get_by_id(id).map(|f| f.parent) else {
                        return;
                    };
                    if let Some(tree_op) = self
                        .tree
                        .apply(file_tree::Action::PasteInto { dest: parent }, &*files)
                    {
                        drop(files);
                        // Re-enter apply as a tree op so MoveInto/CopyInto run.
                        self.apply(ctx, Action::Tree(tree_op));
                    }
                }
                sidebar_views::Op::Delete { id } => {
                    self.open_delete_confirm(vec![id]);
                }
            },
            Action::SelectSidebarPane(pane) => {
                // Active pane while open → dismiss. Otherwise open that pane.
                if self.sidebar_open && self.sidebar_pane == pane {
                    self.sidebar_open = false;
                } else {
                    self.sidebar_pane = pane;
                    self.sidebar_open = true;
                }
            }
            Action::OpenSettings => {
                let Session::Ready(r) = &self.session else {
                    log::info!("settings with no account");
                    return;
                };
                let mut data =
                    settings_panel::load_settings_data(&r.workspace.core, &self.settings);
                // Prefer footer plan if already loaded.
                if data.plan.is_none() {
                    data.plan = r.account.clone();
                }
                self.settings_modal = Some(SettingsModal::new(data));
            }
            Action::NewNote => {
                let Session::Ready(r) = &mut self.session else {
                    log::info!("new note with no workspace");
                    return;
                };
                // Focused parent (selected folder / open tab's parent / root), same
                // as master's "New document" button and Apple quick-create.
                r.workspace.create_doc(false);
            }
            Action::Import => self.import_files(ctx),
            Action::OpenSearch => {
                let Session::Ready(r) = &mut self.session else {
                    log::info!("search with no workspace");
                    return;
                };
                r.workspace.upsert_search(None);
            }
            Action::RequestSync => {
                let Session::Ready(r) = &self.session else {
                    return;
                };
                let core = r.workspace.core.clone();
                let ctx = ctx.clone();
                thread::spawn(move || {
                    if let Err(e) = core.sync() {
                        log::error!("sync failed: {e:?}");
                    }
                    ctx.request_repaint();
                });
            }
        }
    }

    /// Workspace → tree: select, expand ancestors, scroll into view.
    fn reveal_in_tree(&mut self, id: Uuid) {
        let Session::Ready(r) = &self.session else {
            return;
        };
        let files = r.file_cache.read().unwrap();
        self.tree.reveal(id, &*files);
    }

    /// Tree → workspace: create/import parent from cursor (folder, or doc's parent).
    fn sync_focused_parent_from_tree(&mut self) {
        let Session::Ready(r) = &mut self.session else {
            return;
        };
        let parent = {
            let Some(cursor) = self.tree.cursor() else {
                // Leave workspace's own fallbacks (current tab / root).
                r.workspace.focused_parent = None;
                return;
            };
            let files = r.file_cache.read().unwrap();
            let Some(f) = files.get_by_id(cursor) else {
                r.workspace.focused_parent = None;
                return;
            };
            if f.is_folder() { Some(f.id) } else { Some(f.parent) }
        };
        r.workspace.focused_parent = parent;
    }

    /// Copy `get_file_link_url` for `id` to the system clipboard.
    fn copy_file_link(&mut self, ctx: &egui::Context, id: Uuid) {
        let Session::Ready(r) = &self.session else {
            return;
        };
        match r.workspace.core.get_file_link_url(id) {
            Ok(url) => {
                ctx.copy_text(url);
                log::info!("copied file link for {id}");
            }
            Err(e) => log::error!("copy link failed: {e:?}"),
        }
    }

    /// Open the batch share sheet with accepted-collaborator suggestions.
    fn open_share(&mut self, id: Uuid) {
        let Session::Ready(r) = &self.session else {
            self.share_modal = Some(ShareModal::new(id, String::new(), Vec::new()));
            return;
        };
        let me = r.workspace.account.username.clone();
        let files = r.file_cache.read().unwrap();
        let suggested = suggested_collaborators(&files, &me, id);
        drop(files);
        self.share_modal = Some(ShareModal::new(id, me, suggested));
    }

    /// Apply staged shares — one `share_file` per pending person. Closes on full
    /// success; leaves the sheet open with an error if any call fails (earlier
    /// successes stay applied and are dropped from the staged list).
    fn submit_share_batch(&mut self, pending: Vec<modals::PendingShare>) {
        let Some(modal) = self.share_modal.as_ref() else {
            return;
        };
        let id = modal.id;
        let Session::Ready(r) = &self.session else {
            return;
        };
        let core = r.workspace.core.clone();

        let mut failed: Option<(String, String)> = None;
        let mut applied: Vec<String> = Vec::new();
        for p in &pending {
            match core.share_file(id, &p.username, p.mode) {
                Ok(()) => {
                    log::info!("shared {id} with {} ({:?})", p.username, p.mode);
                    applied.push(p.username.to_lowercase());
                }
                Err(e) => {
                    log::error!("share {} failed: {e:?}", p.username);
                    failed = Some((p.username.clone(), format!("{e}")));
                    break;
                }
            }
        }

        if !applied.is_empty() {
            self.rebuild_file_cache();
        }

        if let Some((user, msg)) = failed {
            if let Some(m) = self.share_modal.as_mut() {
                m.pending
                    .retain(|p| !applied.iter().any(|a| p.username.eq_ignore_ascii_case(a)));
                m.error = format!("Couldn’t share with {user}: {msg}");
            }
        } else {
            self.share_modal = None;
        }
    }

    /// Move each id into `parent`, rebuild cache, expand dest, reveal first item.
    fn move_files(&mut self, ids: Vec<Uuid>, parent: Uuid) {
        let Session::Ready(r) = &mut self.session else {
            return;
        };
        let first = ids.first().copied();
        for id in &ids {
            if *id == parent {
                continue;
            }
            // Skip no-ops (already under parent) — workspace would still "succeed".
            let already = r
                .file_cache
                .read()
                .unwrap()
                .get_by_id(*id)
                .is_some_and(|f| f.parent == parent);
            if already {
                continue;
            }
            r.workspace.move_file((*id, parent));
        }
        self.rebuild_file_cache();
        let Session::Ready(r) = &self.session else {
            return;
        };
        let files = r.file_cache.read().unwrap();
        self.tree.prune_missing(&*files);
        // Show the destination and the first moved file.
        if let Some(id) = first {
            self.tree.reveal(id, &*files);
        } else {
            self.tree.reveal(parent, &*files);
        }
        // Stay on Files so the reveal is visible (move often starts from Recents).
        self.sidebar_pane = SidebarPane::Files;
        self.sidebar_open = true;
    }

    /// Native **Save** dialog → write files to disk.
    ///
    /// Dialog + I/O both run on a worker thread so we never block inside
    /// `update` (egui can keep painting). On macOS, rfd hops the panel to the
    /// main queue itself; we still avoid stalling the egui frame that opened
    /// the menu.
    ///
    /// - One document: save panel pre-filled with the note name; bytes → path.
    /// - One folder / multi-select: save panel for the destination; export into it.
    fn export_files(&mut self, ids: Vec<Uuid>) {
        let Session::Ready(r) = &self.session else {
            log::info!("export with no workspace");
            return;
        };
        if ids.is_empty() {
            return;
        }

        // Resolve names / types before spawning (can't hold the cache across it).
        let metas: Vec<(Uuid, String, bool)> = {
            let files = r.file_cache.read().unwrap();
            ids.iter()
                .filter_map(|id| {
                    let f = files.get_by_id(*id)?;
                    Some((*id, f.name.clone(), f.is_folder()))
                })
                .collect()
        };
        if metas.is_empty() {
            return;
        }

        let suggested = if metas.len() == 1 {
            metas[0].1.clone()
        } else {
            "Lockbook Export".into()
        };
        let core = r.workspace.core.clone();
        let ctx = r.workspace.ctx.clone();
        thread::spawn(move || {
            let Some(path) = FileDialog::new().set_file_name(&suggested).save_file() else {
                return;
            };

            if metas.len() == 1 && !metas[0].2 {
                // Single document → write straight to the save path.
                let id = metas[0].0;
                match core.read_document(id, true) {
                    Ok(bytes) => {
                        if let Some(parent) = path.parent() {
                            if let Err(e) = std::fs::create_dir_all(parent) {
                                log::error!("export create parent failed: {e}");
                                ctx.request_repaint();
                                return;
                            }
                        }
                        match std::fs::write(&path, bytes) {
                            Ok(()) => log::info!("exported {id} → {}", path.display()),
                            Err(e) => log::error!("export write failed: {e}"),
                        }
                    }
                    Err(e) => log::error!("export read failed: {e:?}"),
                }
            } else if metas.len() == 1 && metas[0].2 {
                // Single folder: save path is the final folder location.
                // `export_files` writes into dest/<name>; use parent then rename if needed.
                let (id, name, _) = &metas[0];
                let parent = path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                match core.export_files(*id, parent.clone(), true, &None) {
                    Ok(()) => {
                        let written = parent.join(name);
                        if written != path {
                            if let Err(e) = std::fs::rename(&written, &path) {
                                log::error!(
                                    "export ok but rename {} → {}: {e}",
                                    written.display(),
                                    path.display()
                                );
                            }
                        }
                        log::info!("exported folder {id} → {}", path.display());
                    }
                    Err(e) => log::error!("export folder failed: {e:?}"),
                }
            } else {
                // Multi: save path is a new container directory.
                if let Err(e) = std::fs::create_dir_all(&path) {
                    log::error!("export create dir failed: {e}");
                    ctx.request_repaint();
                    return;
                }
                for (id, _, _) in metas {
                    match core.export_files(id, path.clone(), true, &None) {
                        Ok(()) => log::info!("exported {id} → {}", path.display()),
                        Err(e) => log::error!("export {id} failed: {e:?}"),
                    }
                }
            }
            ctx.request_repaint();
        });
    }

    /// Duplicate a file (or folder tree) as a sibling with a unique name.
    fn duplicate_file(&mut self, id: Uuid) {
        let Session::Ready(r) = &self.session else {
            return;
        };
        let core = r.workspace.core.clone();
        match Self::duplicate_into(&core, id, None) {
            Ok(new_id) => {
                log::info!("duplicated {id} → {new_id}");
                self.rebuild_file_cache();
                self.reveal_in_tree(new_id);
            }
            Err(e) => log::error!("duplicate failed: {e}"),
        }
    }

    /// Copy-paste: duplicate each of `ids` under `parent`.
    fn copy_into(&mut self, ids: Vec<Uuid>, parent: Uuid) {
        let Session::Ready(r) = &self.session else {
            return;
        };
        let core = r.workspace.core.clone();
        let mut last = None;
        for id in ids {
            match Self::duplicate_into(&core, id, Some(parent)) {
                Ok(new_id) => {
                    log::info!("copy-pasted {id} → {new_id} under {parent}");
                    last = Some(new_id);
                }
                Err(e) => log::error!("copy-paste {id} failed: {e}"),
            }
        }
        self.rebuild_file_cache();
        if let Some(id) = last {
            self.reveal_in_tree(id);
        } else {
            self.reveal_in_tree(parent);
        }
        self.sidebar_pane = SidebarPane::Files;
        self.sidebar_open = true;
    }

    /// Clone `src` under `dest_parent` (or src's parent when `None`).
    fn duplicate_into(
        core: &Lb, src: Uuid, dest_parent: Option<Uuid>,
    ) -> Result<Uuid, String> {
        let file = core.get_file_by_id(src).map_err(|e| format!("{e:?}"))?;
        let parent = dest_parent.unwrap_or(file.parent);
        let children = core.get_children(&parent).map_err(|e| format!("{e:?}"))?;
        let mut nc = NameComponents::from(&file.name);
        // First copy becomes name-1.ext (or name-1 for folders).
        if nc.variant.is_none() {
            nc.variant = Some(1);
        }
        nc.next_in_children(children);

        let created = core
            .create_file(&nc.to_name(), &parent, file.file_type)
            .map_err(|e| format!("{e:?}"))?;

        if file.file_type == FileType::Document {
            let bytes = core
                .read_document(src, false)
                .map_err(|e| format!("{e:?}"))?;
            core.write_document(created.id, &bytes)
                .map_err(|e| format!("{e:?}"))?;
        } else if file.file_type == FileType::Folder {
            let kids = core.get_children(&src).map_err(|e| format!("{e:?}"))?;
            for kid in kids {
                Self::duplicate_into(core, kid.id, Some(created.id))?;
            }
        }
        Ok(created.id)
    }

    /// Toggle pin in local set + `lb` (optimistic; reverts on API failure).
    fn toggle_pin(&mut self, id: Uuid) {
        let Session::Ready(r) = &mut self.session else {
            return;
        };
        let pinning = !r.pinned.contains(&id);
        if pinning {
            r.pinned.insert(id);
        } else {
            r.pinned.remove(&id);
        }
        let core = r.workspace.core.clone();
        let res = if pinning { core.pin_file(id) } else { core.unpin_file(id) };
        if let Err(e) = res {
            log::error!("pin toggle failed: {e:?}");
            // Revert optimistic update.
            if let Session::Ready(r) = &mut self.session {
                if pinning {
                    r.pinned.remove(&id);
                } else {
                    r.pinned.insert(id);
                }
            }
        }
    }

    /// Finish a folder-picker sheet: move files, or accept a share under `parent`.
    fn confirm_folder_picker(&mut self, purpose: FolderPickerPurpose, parent: Uuid) {
        match purpose {
            FolderPickerPurpose::Move { ids } => self.move_files(ids, parent),
            FolderPickerPurpose::AcceptShare { id, name } => {
                self.accept_share_into(id, name, parent);
            }
        }
    }

    /// Accept a pending share: create a link under `parent` (folder picker),
    /// then rebuild the shared file cache. Local-only.
    fn accept_share_into(&mut self, id: Uuid, name: String, parent: Uuid) {
        let Session::Ready(r) = &self.session else {
            return;
        };
        let path = {
            let files = r.file_cache.read().unwrap();
            link_path_under_parent(&*files, parent, &name)
        };
        match r.workspace.core.create_link_at_path(&path, id) {
            Ok(link) => {
                log::info!("accepted share {} as link {} at {path}", id, link.id);
                self.rebuild_file_cache();
                // Reveal the new link in the Files tree.
                self.reveal_in_tree(link.id);
                self.sidebar_pane = SidebarPane::Files;
                self.sidebar_open = true;
            }
            Err(e) => log::error!("accept share failed: {e:?}"),
        }
    }

    /// Reject a pending share, then rebuild the file cache. Local-only.
    fn reject_share(&mut self, _ctx: &egui::Context, id: Uuid) {
        let Session::Ready(r) = &self.session else {
            return;
        };
        match r.workspace.core.delete_pending_share(&id) {
            Ok(()) => {
                log::info!("rejected share {id}");
                self.rebuild_file_cache();
            }
            Err(e) => log::error!("reject share failed: {e:?}"),
        }
    }

    /// Rebuild the shared `FileCache` after local meta changes (accept/reject).
    fn rebuild_file_cache(&mut self) {
        let Session::Ready(r) = &self.session else {
            return;
        };
        match FileCache::new(&r.workspace.core) {
            Ok(fc) => {
                *r.file_cache.write().unwrap() = fc;
            }
            Err(e) => log::error!("file cache rebuild failed: {e:?}"),
        }
    }

    /// Open a pinned file: documents open in the workspace; folders expand +
    /// select in the tree and switch to the Files pane (Apple pin-chip:
    /// `showFileTree` when a folder chip is tapped from Recents).
    fn open_pinned(&mut self, id: Uuid) {
        let is_folder = {
            let Session::Ready(r) = &self.session else {
                return;
            };
            match r.file_cache.read().unwrap().get_by_id(id) {
                Some(f) => f.is_folder(),
                None => return,
            }
        };
        if is_folder {
            self.reveal_in_tree(id);
            if let Session::Ready(r) = &mut self.session {
                r.workspace.focused_parent = Some(id);
            }
            // Folder chips live above Recents too — jump to the tree so the
            // expand/select is visible (Apple `selectedTab = .files`).
            self.sidebar_pane = SidebarPane::Files;
            self.sidebar_open = true;
        } else if let Session::Ready(r) = &mut self.session {
            r.workspace.open_file(id, true, true);
            self.reveal_in_tree(id);
        }
    }

    /// Native multi-file picker, then `lb.import_files` into the focused parent
    /// (or root). Dialog + import both run on a worker thread so `update` is not
    /// blocked (same pattern as export).
    fn import_files(&mut self, ctx: &egui::Context) {
        let Session::Ready(r) = &self.session else {
            log::info!("import with no workspace");
            return;
        };
        let parent = r.workspace.effective_focused_parent();
        let core = r.workspace.core.clone();
        let files = r.workspace.files.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let Some(paths) = FileDialog::new().pick_files() else {
                return;
            };
            if paths.is_empty() {
                return;
            }

            let result = core.import_files(&paths, parent, &|status| match status {
                ImportStatus::CalculatedTotal(count) => {
                    log::info!("importing {count} files");
                }
                ImportStatus::StartingItem(item) => {
                    log::info!("starting import: {item}");
                }
                ImportStatus::FinishedItem(item) => {
                    log::info!("finished import of {} as lb://{}", item.name, item.id);
                }
            });
            match result {
                Ok(()) => match FileCache::new(&core) {
                    Ok(fc) => {
                        *files.write().unwrap() = fc;
                        log::info!("import complete; file cache rebuilt");
                    }
                    Err(e) => log::error!("import ok but file cache rebuild failed: {e:?}"),
                },
                Err(e) => log::error!("import failed: {e:?}"),
            }
            ctx.request_repaint();
        });
    }

    /// Drive the file tree programmatically — the scripting entry used by
    /// headless observation. Routes through the same `apply` chokepoints a click
    /// would, so a scripted state is reachable-by-hand and vice versa.
    pub fn drive(&mut self, action: file_tree::Action) {
        if let Some(op) = self.tree.apply(action, &self.files) {
            // Headless: no real ctx; demo tree only.
            self.apply(&egui::Context::default(), op.into());
        }
    }

    /// Swap the (currently synthetic) file set — headless observation hook for
    /// driving a specific tree shape.
    pub fn set_files(&mut self, files: Vec<File>) {
        self.files = files;
    }

    /// The no-pixel state projection (see `FileTree::readout`).
    pub fn readout(&self) -> String {
        self.tree.readout(&self.files)
    }
}

/// Absolute path for `create_link_at_path`: link named `name` under `parent`.
fn link_path_under_parent(files: &impl FilesExt, parent: Uuid, name: &str) -> String {
    let Some(folder) = files.get_by_id(parent) else {
        return format!("/{name}");
    };
    if folder.is_root() {
        return format!("/{name}");
    }
    let mut base = files.path(parent);
    // Own-tree folders end with `/`; normalize just in case.
    if !base.ends_with('/') {
        base.push('/');
    }
    if !base.starts_with('/') {
        base.insert(0, '/');
    }
    format!("{base}{name}")
}

/// Known share targets from the **accepted** tree only (never pending-share
/// senders). Ranked by the most recent file they appear on as a collaborator
/// (`last_modified` desc). Excludes `me`, the file **owner**, and anyone with a
/// **direct** share on `file_id`.
///
/// People who only inherit via an ancestor stay in this pool so search can find
/// them (and you can still add a direct share); idle suggestions filter them out
/// separately in the share sheet.
fn suggested_collaborators(
    files: &workspace_rs::file_cache::FileCache, me: &str, file_id: Uuid,
) -> Vec<String> {
    let me = me.to_lowercase();
    let mut on_file: std::collections::HashSet<String> = files
        .get_by_id(file_id)
        .map(|f| {
            f.shares
                .iter()
                .map(|s| s.shared_with.to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    if let Some(owner) = files.get_by_id(file_id).map(|f| f.owner.to_lowercase()) {
        on_file.insert(owner);
    }

    // username → max last_modified among accepted files where they share.
    let mut best: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for f in files.files.values() {
        if f.shares.is_empty() {
            continue;
        }
        for s in &f.shares {
            for name in [&s.shared_with, &s.shared_by] {
                let n = name.to_lowercase();
                if n.is_empty() || n == me || n == "<unknown>" || on_file.contains(&n) {
                    continue;
                }
                best.entry(n)
                    .and_modify(|t| *t = (*t).max(f.last_modified))
                    .or_insert(f.last_modified);
            }
        }
    }

    let mut v: Vec<(String, u64)> = best.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.into_iter().map(|(n, _)| n).collect()
}

/// The sidebar's top block: a reserved toolbar strip (the floating toggle and
/// settings buttons, and on macOS the native traffic lights, sit over it), then
/// Apple-style action chips (New / Import / Search).
fn sidebar_head(ui: &mut egui::Ui, t: &Tokens, actions: &mut Vec<Action>) {
    // Reserve `2 * HEADER_CENTER` for the floating toolbar cluster so the
    // chips start clear of it, plus a little air under the view toggles.
    ui.add_space(HEADER_CENTER * 2.0 + 10.0);
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        let avail = (ui.available_width() - 2.0).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(avail, ui.available_height().max(1.0)),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let (new, import, search) = nav::action_chip_row(ui, t);
                if new {
                    actions.push(Action::NewNote);
                }
                if import {
                    actions.push(Action::Import);
                }
                if search {
                    actions.push(Action::OpenSearch);
                }
            },
        );
    });
}

/// Temp-data key shared with `file_tree` — sticky "sidebar separator is
/// resizing" until primary is released. See `sync_sidebar_resizing_latch`.
const SIDEBAR_RESIZING_LATCH: &str = "lb_sidebar_resizing";

fn sidebar_resize_drag_ids() -> (egui::Id, egui::Id) {
    (
        egui::Id::new("sidebar").with("__resize"),
        egui::Id::new("sidebar_resize_content"),
    )
}

/// Keep a sticky "resizing" flag while primary is down once either resize
/// handle has been dragged. Cleared on pointer-up. The tree reads this so a
/// leftward drag (pointer over the tree while panel width lags a frame)
/// doesn't flash the floating scrollbar.
fn sync_sidebar_resizing_latch(ctx: &egui::Context) {
    let latch = egui::Id::new(SIDEBAR_RESIZING_LATCH);
    if !ctx.input(|i| i.pointer.primary_down()) {
        ctx.data_mut(|d| d.insert_temp(latch, false));
        return;
    }
    let (panel_resize, content_resize) = sidebar_resize_drag_ids();
    if ctx.is_being_dragged(panel_resize) || ctx.is_being_dragged(content_resize) {
        ctx.data_mut(|d| d.insert_temp(latch, true));
    }
}

/// Hit-test strip for the content-side half of the sidebar resize grab.
///
/// egui's `SidePanel` expands the resize sense by `resize_grab_radius_side` on
/// both sides of the edge, but anything drawn later in the content (our
/// workspace `CentralPanel`) steals pointer hits on `[edge, edge+R]`. A thin
/// `Area` after the workspace writes the same `PanelState` SidePanel reads, so
/// drag works from the right of the boundary too without forking width logic.
fn sidebar_resize_over_workspace(ctx: &egui::Context, t: &Tokens) {
    // Keep in sync with SidePanel `.width_range` above.
    const GRAB: f32 = 3.0;
    const WIDTH_MIN: f32 = 268.0;
    const WIDTH_MAX: f32 = 500.0;

    let panel_id = egui::Id::new("sidebar");
    let Some(state) = egui::containers::panel::PanelState::load(ctx, panel_id) else {
        return;
    };

    let edge_x = state.rect.right();
    let rect = egui::Rect::from_min_max(
        egui::pos2(edge_x, state.rect.top()),
        egui::pos2(edge_x + GRAB, state.rect.bottom()),
    );

    // Stable id — latched via `sync_sidebar_resizing_latch` so the tree can
    // keep its floating scrollbar dormant for the whole drag.
    let drag_id = sidebar_resize_drag_ids().1;
    egui::Area::new(drag_id)
        .order(egui::Order::Middle) // above Panel layers, below Foreground chrome
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (_, hit) = ui.allocate_space(rect.size());
            let resp = ui
                .interact(hit, drag_id, egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

            // Same recipe as sticky-header hairlines (`line` × 0.55) — full panel
            // height so elevated stickies don't fuse into the workspace column.
            ui.painter().vline(
                edge_x,
                state.rect.y_range(),
                egui::Stroke::new(1.0, t.line()),
            );

            // SidePanel only paints its hover line from *its* resize response,
            // which never sees content-side hover — firm to full ink while resizing.
            if resp.hovered() || resp.dragged() {
                ui.painter()
                    .vline(edge_x, rect.y_range(), egui::Stroke::new(1.0, t.line()));
            }

            if resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let new_w = (p.x - state.rect.left()).clamp(WIDTH_MIN, WIDTH_MAX);
                    let mut new_rect = state.rect;
                    new_rect.set_right(state.rect.left() + new_w);
                    ui.ctx().data_mut(|d| {
                        d.insert_persisted(
                            panel_id,
                            egui::containers::panel::PanelState { rect: new_rect },
                        );
                    });
                    ui.ctx().request_repaint();
                }
            }
        });
    // Content-side drag is registered after the tree — latch for next frame.
    sync_sidebar_resizing_latch(ctx);
}

/// Windows/Linux borderless resize: thin hit-zones on the window's edges and
/// corners emit `BeginResize` so the host drives a system resize. Foreground
/// order so they sit above the panels at the very border.
#[cfg(not(target_os = "macos"))]
fn window_resize_edges(ctx: &egui::Context) {
    use egui::{CursorIcon as C, PointerButton, ResizeDirection as D, Sense, pos2};

    let r = ctx.screen_rect();
    let b = 6.0; // hit-zone thickness
    let zones = [
        (
            "rz_n",
            egui::Rect::from_min_max(pos2(r.left() + b, r.top()), pos2(r.right() - b, r.top() + b)),
            D::North,
            C::ResizeNorth,
        ),
        (
            "rz_s",
            egui::Rect::from_min_max(
                pos2(r.left() + b, r.bottom() - b),
                pos2(r.right() - b, r.bottom()),
            ),
            D::South,
            C::ResizeSouth,
        ),
        (
            "rz_w",
            egui::Rect::from_min_max(
                pos2(r.left(), r.top() + b),
                pos2(r.left() + b, r.bottom() - b),
            ),
            D::West,
            C::ResizeWest,
        ),
        (
            "rz_e",
            egui::Rect::from_min_max(
                pos2(r.right() - b, r.top() + b),
                pos2(r.right(), r.bottom() - b),
            ),
            D::East,
            C::ResizeEast,
        ),
        (
            "rz_nw",
            egui::Rect::from_min_max(r.min, pos2(r.left() + b, r.top() + b)),
            D::NorthWest,
            C::ResizeNwSe,
        ),
        (
            "rz_ne",
            egui::Rect::from_min_max(pos2(r.right() - b, r.top()), pos2(r.right(), r.top() + b)),
            D::NorthEast,
            C::ResizeNeSw,
        ),
        (
            "rz_sw",
            egui::Rect::from_min_max(
                pos2(r.left(), r.bottom() - b),
                pos2(r.left() + b, r.bottom()),
            ),
            D::SouthWest,
            C::ResizeNeSw,
        ),
        (
            "rz_se",
            egui::Rect::from_min_max(pos2(r.right() - b, r.bottom() - b), r.max),
            D::SouthEast,
            C::ResizeNwSe,
        ),
    ];
    for (id, rect, dir, cursor) in zones {
        egui::Area::new(egui::Id::new(id))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let resp = ui
                    .allocate_response(rect.size(), Sense::drag())
                    .on_hover_cursor(cursor);
                if resp.drag_started_by(PointerButton::Primary) {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                }
            });
    }
}

#[cfg(feature = "egui_wgpu_renderer")]
mod lb_wgpu {
    use egui::{PlatformOutput, ViewportIdMap, ViewportOutput};
    use egui_wgpu_renderer::RendererState;

    use crate::{Lockbook, Response};

    #[repr(C)]
    pub struct WgpuLockbook<'window> {
        pub renderer: RendererState<'window>,

        // events for the subsequent two frames, because canvas expects buttons to be down for two frames
        pub queued_events: Vec<egui::Event>,
        pub double_queued_events: Vec<egui::Event>,

        pub app: Lockbook,
    }

    #[derive(Default)]
    pub struct Output {
        // platform response
        pub platform: PlatformOutput,
        pub viewport: ViewportIdMap<ViewportOutput>,

        // widget response
        pub app: Response,
    }

    impl WgpuLockbook<'_> {
        pub fn frame(&mut self) -> Output {
            self.renderer.begin_frame();
            let app_response = self.app.update(&self.renderer.context);
            self.renderer.set_is_dev(self.app.is_dev());
            let (platform, viewport) = self.renderer.end_frame();

            // Queue up the events for the next frame
            self.renderer
                .raw_input
                .events
                .append(&mut self.queued_events);
            self.queued_events.append(&mut self.double_queued_events);
            if !self.renderer.raw_input.events.is_empty() {
                self.renderer.context.request_repaint();
            }

            Output { platform, viewport, app: app_response }
        }
    }
}
