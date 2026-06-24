use std::sync::Arc;

use lb_rs::{
    Uuid,
    blocking::Lb,
    model::{
        core_config::Config,
        svg::element::{Color as SvgColor, DynamicColor},
    },
};
use workspace_rs::{
    resolvers::image_embed::ImageEmbedResolver,
    tab::{
        markdown_editor::{Editor, HttpClient, MdConfig, MdResources},
        svg_editor::SVGEditor,
    },
    theme::palette_v2::{Mode, Theme, ThemeExt as _},
    widgets::image_cache::ImageCache,
    workspace::WsPersistentStore,
};

pub struct LbWebApp {
    core: Lb,
    cfg: WsPersistentStore,
    images: Option<ImageCache>,
    editor: Option<Editor>,
    canvas: Option<SVGEditor>,
    initial_screen: InitialScreen,
    /// Last page mode we synced visuals to. None until first update, then
    /// holds Some(mode) so we only re-set theme when the page toggles.
    last_mode: Option<Mode>,
}

#[derive(PartialEq, Clone, Copy)]
pub enum InitialScreen {
    Canvas,
    Editor,
}

impl InitialScreen {
    /// CSS variable whose value the WASM should paint as its surface bg.
    /// Editor reads `--bg-sunken` to match the gray chrome around it;
    /// Canvas reads `--demo-bg` which CSS resolves to white (light) or
    /// the deep `--bg-sunken` (dark) — pure paper feel in light mode,
    /// no light surface stuck against a dark page in dark mode.
    fn bg_css_var(self) -> &'static str {
        match self {
            InitialScreen::Canvas => "--demo-bg",
            InitialScreen::Editor => "--bg-sunken",
        }
    }
}

/// Reads the current page mode from the document.
/// `<html class="dark">` → Mode::Dark; otherwise Mode::Light.
#[cfg(target_arch = "wasm32")]
fn current_page_mode() -> Mode {
    eframe::web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .map(|el| {
            if el
                .class_name()
                .split_ascii_whitespace()
                .any(|c| c == "dark")
            {
                Mode::Dark
            } else {
                Mode::Light
            }
        })
        .unwrap_or(Mode::Dark)
}

#[cfg(not(target_arch = "wasm32"))]
fn current_page_mode() -> Mode {
    Mode::Dark
}

/// Reads the named CSS variable off the document root as a `Color32`. The
/// token is stored as the space-separated RGB triplet that Tailwind uses
/// with `rgb(var(--name))`, so we parse three u8s out of the computed
/// string. The chosen var threads through to `code_bg_color`,
/// `panel_fill`, and the SVG canvas's `background_color` so the WASM
/// surface matches the CSS-painted chrome around it without seams.
#[cfg(target_arch = "wasm32")]
fn page_bg(var: &str) -> egui::Color32 {
    use eframe::web_sys::window;
    let fallback = egui::Color32::from_gray(16); // dark-mode-ish fallback
    let Some(win) = window() else { return fallback };
    let Some(doc) = win.document() else { return fallback };
    let Some(el) = doc.document_element() else { return fallback };
    let Ok(Some(style)) = win.get_computed_style(&el) else { return fallback };
    let Ok(val) = style.get_property_value(var) else { return fallback };
    let parts: Vec<u8> = val
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u8>().ok())
        .collect();
    if parts.len() >= 3 {
        egui::Color32::from_rgb(parts[0], parts[1], parts[2])
    } else {
        fallback
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn page_bg(_var: &str) -> egui::Color32 {
    egui::Color32::from_gray(16)
}

/// Installs the workspace_rs theme for `mode` and then flattens the theme's
/// three-tier surface palette down to a single tone (`page_bg()` =
/// `--bg-sunken`). The native lockbook clients keep the tiers distinct
/// (`neutral_bg` < `neutral_bg_secondary` < `neutral_bg_tertiary`) so code
/// blocks, panels, and widget chrome stand apart from the editor canvas. The
/// public-site demos want one continuous surface that matches the
/// `bg-sunken` CSS containers wrapping them, so we collapse the upper tiers
/// onto the canvas tier here.
///
/// The flow is: `set_lb_theme` populates visuals from `theme.base_visuals()`
/// (private to workspace_rs), which sets `extreme_bg_color = neutral_bg()`
/// and selection/hyperlink/widget tones from the lockbook palette. We then
/// read those visuals back, overwrite just the three surface fields that
/// power editor/canvas backgrounds, and re-apply. Selection blue,
/// hyperlinks, and widget hover/active tones stay theme-derived.
fn apply_theme_and_flatten_surfaces(ctx: &egui::Context, mode: Mode, bg: egui::Color32) {
    ctx.set_lb_theme(Theme::default(mode));

    let mut v = ctx.style().visuals.clone();
    v.widgets.noninteractive.bg_fill = bg;
    v.code_bg_color = bg;
    v.panel_fill = bg;
    // extreme_bg_color is already `theme.neutral_bg()` via base_visuals();
    // by intentional CSS↔theme alignment that equals `page_bg()`. Leaving
    // it as-set keeps the editor's `Frame::canvas` viewport on the canvas
    // tier where it belongs.
    ctx.set_visuals(v);
}

fn sync_canvas_bg(canvas: &mut SVGEditor, bg: egui::Color32) {
    let c = SvgColor::new_rgb(bg.r(), bg.g(), bg.b());
    canvas.settings.background_color = DynamicColor { light: c, dark: c };
}

impl LbWebApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, initial_screen: InitialScreen) -> Self {
        let ctx = cc.egui_ctx.clone();

        let lb = Lb::init(Config {
            logs: true,
            colored_logs: false,
            writeable_path: "".into(),
            background_work: false,
            stdout_logs: true,
        })
        .unwrap();

        let mut fonts = egui::FontDefinitions::default();

        workspace_rs::register_fonts(&mut fonts);

        ctx.set_fonts(fonts);
        ctx.set_zoom_factor(0.9);

        let initial_mode = current_page_mode();
        apply_theme_and_flatten_surfaces(&ctx, initial_mode, page_bg(initial_screen.bg_css_var()));

        let Some(ref wgpu) = cc.wgpu_render_state else {
            panic!("must use wgpu as graphics target")
        };

        workspace_rs::register_render_callback_resources(
            &wgpu.device,
            &wgpu.queue,
            wgpu.target_format,
            &mut wgpu.renderer.write(),
            workspace_rs::register_font_system(&ctx),
            1,
        );

        let cfg = WsPersistentStore::new(false, "/tmp/lb-public-site".into());

        Self {
            core: lb,
            cfg,
            images: None,
            editor: None,
            canvas: None,
            initial_screen,
            last_mode: Some(initial_mode),
        }
    }
}

impl eframe::App for LbWebApp {
    /// Clear color for the wgpu render pass. Defaults to a semi-transparent
    /// #0C0C0C which leaks through any pixel not covered by a `rect_filled`.
    /// Threading `page_bg()` through here keeps the cleared color identical
    /// to the painted bg (light: #FFFFFF, dark: #101010) so the demo never
    /// shows the default dark gray at the edges.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let bg = page_bg(self.initial_screen.bg_css_var());
        egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 255)
            .to_normalized_gamma_f32()
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Re-sync theme to the page mode only when it changes (avoids
        // recreating egui visuals every frame). Then poll lightly so a
        // theme toggle while the demo is idle still gets picked up.
        let mode = current_page_mode();
        if self.last_mode != Some(mode) {
            let bg = page_bg(self.initial_screen.bg_css_var());
            apply_theme_and_flatten_surfaces(ctx, mode, bg);
            if let Some(canvas) = self.canvas.as_mut() {
                sync_canvas_bg(canvas, bg);
            }
            self.last_mode = Some(mode);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.widgets.noninteractive.bg_fill))
            .show(ctx, |ui| {
                if self.editor.is_none() && self.initial_screen == InitialScreen::Editor {
                    let files = Arc::new(std::sync::RwLock::new(
                        workspace_rs::file_cache::FileCache::empty(),
                    ));
                    let file_id = Uuid::new_v4();
                    let images = ImageCache::new(
                        ctx.clone(),
                        HttpClient::default(),
                        self.core.clone(),
                        Arc::clone(&files),
                        self.cfg.clone(),
                    );
                    self.images = Some(images.clone());
                    self.editor = Some(Editor::new(
                        include_str!("../resources/editor-demo.md"),
                        file_id,
                        None,
                        MdResources {
                            ctx: ctx.clone(),
                            core: self.core.clone(),
                            persistence: self.cfg.clone(),
                            files,
                            link_resolver: Box::new(()),
                            embeds: Box::new(ImageEmbedResolver::new(images, file_id)),
                        },
                        MdConfig { readonly: false, ext: "md".into(), tablet_or_desktop: true },
                    ));
                }

                if self.canvas.is_none() && self.initial_screen == InitialScreen::Canvas {
                    let svg = include_str!("../resources/canvas-demo.svg");
                    let mut canvas = SVGEditor::new(
                        svg.as_bytes(),
                        ui.ctx(),
                        self.core.clone(),
                        Uuid::new_v4(),
                        None,
                        &self.cfg,
                        false,
                    );
                    sync_canvas_bg(&mut canvas, page_bg(self.initial_screen.bg_css_var()));
                    self.canvas = Some(canvas);
                }
                if let Some(images) = &self.images {
                    images.begin_frame();
                }
                if let Some(md) = &mut self.editor {
                    egui::Frame::default().show(ui, |ui| {
                        md.show(ui);
                    });
                }
                if let Some(images) = &self.images {
                    images.end_frame();
                }

                if let Some(svg) = &mut self.canvas {
                    egui::Frame::default().show(ui, |ui| {
                        svg.show(ui);
                    });
                }
            });
    }
}
