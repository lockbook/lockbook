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

use lb::model::file::File;
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};
use workspace_rs::theme::visuals;

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
        }
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

        // The toggle and settings buttons float as a fixed top-left cluster so
        // they stay put whether the sidebar is open or closed.
        egui::Area::new("sidebar_toolbar".into())
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

        // Center: the design system, in place of the workspace for now.
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(t.canvas()))
            .show(ctx, |ui| {
                ui.add_space(20.0);
                egui::Frame::default()
                    .inner_margin(egui::Margin::symmetric(60, 12))
                    .show(ui, |ui| design_system::show(ctx, ui, &t, &mut self.mode));
            });

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
        let files = &self.files;
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
                        if let Some(op) = tree.show(ui, t, files) {
                            actions.push(op.into());
                        }
                    });
            });
    }

    /// The one place shell state changes. Once the workspace is wired in, the
    /// model mutations here become `Workspace` calls.
    fn apply(&mut self, action: Action) {
        match action {
            Action::Tree(file_tree::Op::Open { id, new_tab }) => {
                log::info!("open file {id} (new_tab={new_tab})");
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
        if let Some(op) = self.tree.apply(action) {
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
