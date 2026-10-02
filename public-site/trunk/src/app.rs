use std::sync::Arc;

use lb_rs::{
    Uuid,
    blocking::Lb,
    model::core_config::{ClientType, Config},
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
    /// Last light/dark mode applied, so `update` only reinstalls the theme
    /// when the page's mode actually changes.
    mode: Mode,
}

/// The site resolves its own light/dark state — explicit toggle, else the OS
/// preference — into `<html data-lb-mode="light|dark">`. Reading one attribute
/// keeps the demos in step with the page without any wasm-bindgen plumbing.
/// Defaults to dark so a standalone trunk build still looks right.
#[cfg(target_arch = "wasm32")]
fn page_mode() -> Mode {
    use eframe::web_sys;
    let attr = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|e| e.get_attribute("data-lb-mode"));
    match attr.as_deref() {
        Some("light") => Mode::Light,
        _ => Mode::Dark,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn page_mode() -> Mode {
    Mode::Dark
}

#[derive(PartialEq)]
pub enum InitialScreen {
    Canvas,
    Editor,
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
            client_type: ClientType::Ui,
        })
        .unwrap();

        let mut fonts = egui::FontDefinitions::default();

        workspace_rs::register_fonts(&mut fonts);

        ctx.set_fonts(fonts);
        ctx.set_zoom_factor(0.9);

        // set_lb_theme installs mode-correct visuals itself (Theme::base_visuals),
        // so there is nothing to override afterwards.
        let mode = page_mode();
        ctx.set_lb_theme(Theme::default(mode));

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

        let cfg = WsPersistentStore::new(false, "/tmp/lb-public-site".into(), true);

        Self { core: lb, cfg, images: None, editor: None, canvas: None, initial_screen, mode }
    }
}

impl eframe::App for LbWebApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mode = page_mode();
        if mode != self.mode {
            self.mode = mode;
            ctx.set_lb_theme(Theme::default(mode));
        }

        // Fill with neutral_bg — the tab/paper surface, matching what the site
        // paints behind the canvas — so the demo blends into its window frame.
        let paper = ctx.get_lb_theme().neutral_bg();
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(paper))
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
                    self.canvas = Some(SVGEditor::new(
                        svg.as_bytes(),
                        ui.ctx(),
                        self.core.clone(),
                        Uuid::new_v4(),
                        None,
                        &self.cfg,
                        false,
                    ))
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
