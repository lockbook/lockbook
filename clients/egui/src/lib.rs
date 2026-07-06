#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod design_system;
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

use lb::blocking::Lb;
use lb::model::core_config::Config;
use lb::model::file::File;
use workspace_rs::file_cache::FileCache;
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};
use workspace_rs::theme::visuals;
use workspace_rs::workspace::Workspace;

use crate::theme::icons;
use crate::theme::tokens::Tokens;
use crate::widgets::file_tree::FileTree;
use crate::widgets::nav;

/// The sidebar extends to the top of the window; a small toolbar cluster (toggle
/// then settings) floats at its top-left, always visible. `HEADER_CENTER` is
/// the y-center that row (and the floating toggle) align to — the center of the
/// native macOS traffic lights (dev), measured at ~16pt from the window top via
/// an on-screen ruler. Our icons' centers line up with the lights' centers. On
/// macOS the lights sit top-left, so the toggle is pushed clear of them; tune
/// `TOGGLE_X` to taste. Elsewhere it's a normal inset.
const HEADER_CENTER: f32 = 16.0;
#[cfg(target_os = "macos")]
const TOGGLE_X: f32 = 76.0;
#[cfg(not(target_os = "macos"))]
const TOGGLE_X: f32 = 10.0;

pub struct Lockbook {
    mode: Mode,
    tree: FileTree,
    files: Vec<File>,
    sidebar_open: bool,
    session: Session,
}

/// The account lifecycle. `Demo` is the headless/observe default and the state
/// before `start_core` runs; `Loading` awaits the off-thread `lb` init; `Ready`
/// holds the live workspace and file cache once signed in. Signed-out (no
/// account) falls back to `Demo`'s placeholder until onboarding is built.
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
    ToggleSidebar,
    OpenSettings,
    NewNote,
    OpenSearch,
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
        let mode = if ctx.style().visuals.dark_mode { Mode::Dark } else { Mode::Light };
        Lockbook {
            mode,
            tree: FileTree::default(),
            files: file_tree::demo_files(),
            sidebar_open: true,
            session: Session::Demo,
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
    pub fn deferred_init(&self, ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        workspace_rs::register_fonts(&mut fonts);
        theme::icons::register(&mut fonts);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(ctx);

        let mode = if ctx.style().visuals.dark_mode { Mode::Dark } else { Mode::Light };
        ctx.set_lb_theme(Theme::default(mode));
        visuals::init(ctx);
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
                            let workspace =
                                Workspace::new(&core, ctx, true, Some(file_cache.clone()));
                            Session::Ready(Box::new(Ready { file_cache, workspace }))
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

        // egui draws the sidebar resize handle with the "highly visible"
        // hovered/active foreground strokes; soften both to a faint hairline,
        // then restore so no other widget is affected.
        let resize_stroke = egui::Stroke::new(1.0, t.line());
        let saved = {
            let s = ctx.style();
            (s.visuals.widgets.hovered.fg_stroke, s.visuals.widgets.active.fg_stroke)
        };
        ctx.style_mut(|s| {
            s.visuals.widgets.hovered.fg_stroke = resize_stroke;
            s.visuals.widgets.active.fg_stroke = resize_stroke;
        });
        if self.sidebar_open {
            self.sidebar(ctx, &t, &mut actions);
        }
        ctx.style_mut(|s| {
            (s.visuals.widgets.hovered.fg_stroke, s.visuals.widgets.active.fg_stroke) = saved;
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

        // The toggle and settings buttons float as a fixed top-left cluster.
        // Foreground order (drawn after the drag strip and resize zones) keeps
        // their clicks from being swallowed by the title-bar drag region beneath.
        egui::Area::new("sidebar_toolbar".into())
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(TOGGLE_X, HEADER_CENTER - nav::ICON_BUTTON_SIZE / 2.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if nav::icon_button(ui, &t, icons::SIDEBAR).clicked() {
                        actions.push(Action::ToggleSidebar);
                    }
                    if nav::icon_button(ui, &t, icons::GEAR).clicked() {
                        actions.push(Action::OpenSettings);
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

        // Center: the live workspace once signed in; otherwise the design system
        // placeholder (also shown while loading and — until onboarding exists —
        // when signed out).
        if let Session::Ready(r) = &mut self.session {
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(t.canvas()))
                .show(ctx, |ui| {
                    r.workspace.show(ui);
                });
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(t.canvas()))
                .show(ctx, |ui| {
                    ui.add_space(20.0);
                    egui::Frame::default()
                        .inner_margin(egui::Margin::symmetric(60, 12))
                        .show(ui, |ui| design_system::show(ctx, ui, &t, &mut self.mode));
                });
        }

        for action in actions {
            self.apply(action);
        }
        Response { close: ctx.input(|i| i.viewport().close_requested()) }
    }

    /// The sidebar, top to bottom. Fixed top (toolbar + quick actions) and bottom
    /// (status, usage) reserve their space as nested panels; the file tree fills
    /// the remainder as the `CentralPanel` — which is exactly why it sizes to
    /// what's left, and why it must draw last.
    fn sidebar(&mut self, ctx: &egui::Context, t: &Tokens, actions: &mut Vec<Action>) {
        let tree = &mut self.tree;
        let session = &self.session;
        let demo = &self.files;
        egui::SidePanel::left("sidebar")
            .resizable(true)
            // Convergent desktop defaults (see reference_file_tree_sidebar_metrics):
            // 240px default, 180px floor, ~480px ceiling.
            .default_width(240.0)
            .width_range(180.0..=480.0)
            .show_separator_line(false)
            .frame(egui::Frame::default().fill(t.surface()))
            .show(ctx, |ui| {
                egui::TopBottomPanel::top("sidebar_head")
                    .show_separator_line(false)
                    // No top inset: the header row starts at y=0 so its settings
                    // button centers on `HEADER_CENTER` (level with the toggle).
                    .frame(egui::Frame::default().inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 0,
                        bottom: 14,
                    }))
                    .show_inside(ui, |ui| sidebar_head(ui, t, actions));
                egui::TopBottomPanel::bottom("sidebar_foot")
                    .show_separator_line(false)
                    .frame(egui::Frame::default().inner_margin(egui::Margin::same(8)))
                    .show_inside(ui, |ui| sidebar_foot(ui, t));
                egui::CentralPanel::default()
                    .frame(egui::Frame::default())
                    .show_inside(ui, |ui| {
                        // Live file cache when signed in; demo files otherwise.
                        let op = match session {
                            Session::Ready(r) => {
                                let files = r.file_cache.read().unwrap();
                                tree.show(ui, t, &*files)
                            }
                            _ => tree.show(ui, t, demo),
                        };
                        if let Some(op) = op {
                            actions.push(op.into());
                        }
                    });
            });
    }

    /// The one place shell state changes. Once the workspace is wired in, the
    /// model mutations here become `Workspace` calls.
    fn apply(&mut self, action: Action) {
        match action {
            Action::Tree(op) => {
                let Session::Ready(r) = &mut self.session else {
                    log::info!("tree op with no workspace: {op:?}");
                    return;
                };
                match op {
                    file_tree::Op::Open { id, new_tab } => r.workspace.open_file(id, true, new_tab),
                    file_tree::Op::CreateDoc { parent } => r.workspace.create_doc_at(false, parent),
                    file_tree::Op::CreateFolder { parent } => r.workspace.create_folder_at(parent),
                    file_tree::Op::Delete { ids } => {
                        for id in ids {
                            r.workspace.delete_file(id);
                        }
                    }
                }
            }
            Action::ToggleSidebar => self.sidebar_open = !self.sidebar_open,
            Action::OpenSettings => log::info!("open settings"),
            Action::NewNote => log::info!("new note"),
            Action::OpenSearch => log::info!("open search"),
        }
    }

    /// Drive the file tree programmatically — the scripting entry used by
    /// headless observation. Routes through the same `apply` chokepoints a click
    /// would, so a scripted state is reachable-by-hand and vice versa.
    pub fn drive(&mut self, action: file_tree::Action) {
        if let Some(op) = self.tree.apply(action, &self.files) {
            self.apply(op.into());
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

/// The sidebar's top block: a reserved toolbar strip (the floating toggle and
/// settings buttons, and on macOS the native traffic lights, sit over it), then
/// quick-action rows (tree-styled), then a section hairline.
fn sidebar_head(ui: &mut egui::Ui, t: &Tokens, actions: &mut Vec<Action>) {
    // Reserve `2 * HEADER_CENTER` for the floating toolbar cluster so the
    // quick-action rows start clear of it.
    ui.add_space(HEADER_CENTER * 2.0);

    if nav::nav_row(ui, t, icons::NOTE_PENCIL, "New note").clicked() {
        actions.push(Action::NewNote);
    }
    if nav::nav_row(ui, t, icons::SEARCH, "Search").clicked() {
        actions.push(Action::OpenSearch);
    }
    // The nav rows are taller than tree rows, so their built-in bottom padding
    // already sits above the hairline; this gap plus the head panel's bottom
    // margin keep equal space above and below the divider.
    ui.add_space(11.0);
    nav::hairline(ui, t);
}

/// The sidebar's bottom block: a section hairline, sync status, and a storage
/// usage indicator. Placeholder data until the workspace is wired in.
fn sidebar_foot(ui: &mut egui::Ui, t: &Tokens) {
    nav::hairline(ui, t);
    ui.add_space(10.0);
    status_line(ui, t, icons::CLOUD_CHECK, "Up to date");
    ui.add_space(8.0);
    usage_bar(ui, t, 0.42, "2.1 GB of 5 GB");
}

/// A non-interactive icon + label line (sync status).
fn status_line(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::hover());
    let cy = rect.center().y;
    let ink = t.text_muted();
    let mut x = rect.left() + 6.0;
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(15.0), ink);
    ui.painter()
        .galley(egui::pos2(x, cy - g.size().y / 2.0), g, ink);
    x += 22.0;
    let g = ui
        .painter()
        .layout_no_wrap(label.into(), egui::FontId::proportional(13.0), ink);
    ui.painter()
        .galley(egui::pos2(x, cy - g.size().y / 2.0), g, ink);
}

/// A thin storage-usage bar with a caption above it.
fn usage_bar(ui: &mut egui::Ui, t: &Tokens, frac: f32, label: &str) {
    let inset = 6.0;
    let (lr, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 15.0), egui::Sense::hover());
    let g =
        ui.painter()
            .layout_no_wrap(label.into(), egui::FontId::monospace(11.0), t.text_faint());
    ui.painter().galley(
        egui::pos2(lr.left() + inset, lr.center().y - g.size().y / 2.0),
        g,
        t.text_faint(),
    );

    ui.add_space(5.0);
    let (br, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::hover());
    let track = egui::Rect::from_min_size(
        egui::pos2(br.left() + inset, br.top()),
        egui::vec2(br.width() - 2.0 * inset, 5.0),
    );
    ui.painter()
        .rect_filled(track, 2.5, t.fg().gamma_multiply(0.10));
    let fill =
        egui::Rect::from_min_size(track.min, egui::vec2(track.width() * frac.clamp(0.0, 1.0), 5.0));
    ui.painter()
        .rect_filled(fill, 2.5, t.fg().gamma_multiply(0.40));
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
