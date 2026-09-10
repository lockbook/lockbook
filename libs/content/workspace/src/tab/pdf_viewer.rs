use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use crate::style::{
    CHROME_BAND_GLYPH, CHROME_BAND_H, Radius, ThemeExt, TypeRole, control_height,
    icon_button_glyph, interact_fill_response, loading_indicator, phosphor, quiet_canvas_fills,
    sense_click, tip_text, with_overlay_scroll,
};
use egui::{
    Align, CentralPanel, ColorImage, Context, Event, Id, Image, ImageSource, Key, Modifiers, Pos2,
    Rect, ScrollArea, SidePanel, TextureHandle, Ui, Vec2, load::SizedTexture,
};
use hayro::{InterpreterSettings, Pdf, RenderSettings};
use lb_rs::Uuid;
use web_time::Instant;

pub struct PdfViewer {
    pub id: Uuid,

    ctx: Context,

    page_dimensions: Vec<(f32, f32)>,

    parse_failed: bool,

    /// When the tab opened — initial-load spinner waits 200ms (same as search).
    opened: Instant,

    /// the bounds of all the pages, as influenced by scale. Includes a safe-area as
    /// a last page to address some shortcommings with egui::ScrollArea not being able
    /// scroll beyond what it thinks the bottom of the last page is. I was not able
    /// to get it to be aware of additional space in the frame that the re-scale happens.
    page_bounds: Vec<Rect>,

    page_cache: HashMap<usize, CachedPage>,

    thumbnail_cache: HashMap<usize, TextureHandle>,

    placeholder: TextureHandle,

    generation: Arc<AtomicU64>,

    requested: HashSet<(usize, RenderKind, Generation)>,

    request_tx: Sender<WorkerRequest>,
    response_rx: Receiver<WorkerResponse>,

    /// The current scale 1 == 100% zoom
    scale: f32,

    /// if true, at the top of the frame, scale will be adjusted to fit the available
    /// width (or height)
    fit_width: bool,

    /// see [Self::fit_width]
    fit_height: bool,

    /// the current page in the viewport, calculated by what page has the most visible area
    /// ties are broken by the lowest index. Gets a lil strange when you zoom out and are view
    /// ing the bottom pages, but I think it makes sense overall and has the least weird stuff
    /// in general. Can tweak the algo just a bit to get rid of that last problem
    current_page: usize,

    /// what page should we scroll to, generally set by the sidebar, though it could be cool
    /// to link to pages from md docs or something at some point
    scroll_to: Option<usize>,

    /// where is the viewport rendered? needed for scale computations to identify the
    /// location of the cursor in page-space
    render_area: Rect,

    /// what is the scroll area looking at right now? See also [Self::render_area]
    current_viewport: Rect,

    /// if we just scrolled, what should our new viewport be? This value is annoying to calculate
    /// and deal with. The scroll area is fickle about when it receives this information and when
    /// it actually does the rendering. An ideal scroll area would be able to process this
    /// immediately and keep animations on, and also not require a safe area at the bottom
    viewport_adjustment: Option<Vec2>,

    sidebar: Option<SideBar>,

    /// Host size class (iPhone / iPad compact). Not inferred from width or OS.
    /// Workspace sets this each frame from [`crate::workspace::Workspace::desktop_tab_policy`].
    pub(crate) compact: bool,
}

struct SideBar {
    thumbnails: Vec<Content>,
    is_visible: bool,
    scroll_target: usize,
}

#[derive(Clone, Copy)]
struct Content {
    size: Vec2,
}

type Generation = u64;

#[derive(Clone)]
struct CachedPage {
    texture: TextureHandle,
    generation: Generation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RenderKind {
    Page,
    Thumbnail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PdfCmd {
    ZoomIn,
    ZoomOut,
    Reset,
    FitWidth,
    FitHeight,
    ToggleSidebar,
}

enum WorkerRequest {
    Render { page_idx: usize, kind: RenderKind, scale: f32, generation: Generation },
}

enum WorkerResponse {
    Parsed { page_dimensions: Vec<(f32, f32)> },
    ParseFailed,
    Rendered { page_idx: usize, kind: RenderKind, generation: Generation, image: ColorImage },
}

const ZOOM_STOP: f32 = 0.1;
const SIDEBAR_WIDTH: f32 = 230.0;
const SPACE_BETWEEN_PAGES: f32 = 10.0;
const THUMBNAIL_SCALE: f32 = 0.15;
const LOAD_SPINNER_DELAY: f32 = 0.20;

/// Flush zoom strip at the top of `host`; pages + sidebar fill the rest.
///
/// SidePanel / CentralPanel consume leftover and clip to it. Painting the
/// strip after them lands in a 0-height sliver at the bottom (invisible).
fn pdf_bands(host: Rect) -> (Rect, Rect) {
    let h = CHROME_BAND_H.min(host.height().max(0.0));
    let split_y = host.top() + h;
    let toolbar = Rect::from_min_max(host.min, egui::pos2(host.right(), split_y));
    let body = Rect::from_min_max(egui::pos2(host.left(), split_y), host.max);
    (toolbar, body)
}

fn toolbar_icon(
    ui: &mut Ui, t: &crate::style::Theme, icon: &'static str, ground: egui::Color32, hit: f32,
    tip: &str,
) -> bool {
    let r = icon_button_glyph(ui, t, icon, true, ground, hit, CHROME_BAND_GLYPH);
    tip_text(ui.ctx(), &r, tip);
    r.clicked()
}

impl PdfViewer {
    pub fn new(id: Uuid, bytes: Vec<u8>, ctx: &egui::Context) -> Self {
        let bytes: Arc<Vec<u8>> = Arc::new(bytes);
        let (request_tx, request_rx) = mpsc::channel::<WorkerRequest>();
        let (response_tx, response_rx) = mpsc::channel::<WorkerResponse>();
        let generation = Arc::new(AtomicU64::new(0));
        spawn_worker(bytes, request_rx, response_tx, ctx.clone(), generation.clone());

        let placeholder = ctx.load_texture(
            "pdf_placeholder",
            ColorImage::from_rgba_premultiplied([1, 1], &[255, 255, 255, 255]),
            egui::TextureOptions::LINEAR,
        );

        Self {
            id,
            sidebar: Default::default(),
            page_dimensions: Default::default(),
            parse_failed: false,
            opened: Instant::now(),
            page_cache: Default::default(),
            thumbnail_cache: Default::default(),
            placeholder,
            generation,
            requested: Default::default(),
            request_tx,
            response_rx,
            page_bounds: Default::default(),
            ctx: ctx.clone(),
            scale: 1.,
            fit_width: true,
            scroll_to: None,
            current_page: 0,
            fit_height: false,
            current_viewport: Rect::ZERO,
            viewport_adjustment: Default::default(),
            render_area: Rect::ZERO,
            compact: false,
        }
    }

    fn setup_sidebar(&mut self) {
        let inner_width = SIDEBAR_WIDTH - 50.0;

        let thumbnails = self
            .page_dimensions
            .iter()
            .map(|&(w, h)| {
                let aspect = if w > 0. { h / w } else { 1. };
                Content { size: egui::vec2(inner_width, inner_width * aspect) }
            })
            .collect();

        self.sidebar = Some(SideBar { thumbnails, is_visible: false, scroll_target: 0 });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.drain_responses();
        // Workspace `visuals::apply` restores default spacing / 17pt. DS chrome
        // assumes 0 item_spacing (same as search / desktop context).
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let t = ui.ctx().get_lb_theme();

        let host = ui.available_rect_before_wrap();
        ui.painter().rect_filled(host, 0., t.neutral_bg());

        // Tab content is shown inside `centered_and_justified`. Own a top-down
        // column so the flush strip is reserved before SidePanel/CentralPanel
        // eat leftover (and clip to a 0-height sliver).
        if !self.compact && self.sidebar.is_none() && !self.page_dimensions.is_empty() {
            self.setup_sidebar();
        }

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let host = ui.available_rect_before_wrap();
            let (toolbar, _) = pdf_bands(host);
            crate::style::claim(ui, toolbar);
            self.show_toolbar(ui, toolbar);
            if !self.compact {
                self.show_sidebar(ui);
            }
            self.show_pages(ui);
        });
    }

    fn drain_responses(&mut self) {
        while let Ok(resp) = self.response_rx.try_recv() {
            match resp {
                WorkerResponse::Parsed { page_dimensions } => {
                    self.page_dimensions = page_dimensions;
                    self.compute_bounds();
                    if !self.compact {
                        self.setup_sidebar();
                    }
                }
                WorkerResponse::ParseFailed => {
                    self.parse_failed = true;
                }
                WorkerResponse::Rendered { page_idx, kind, generation, image } => {
                    self.requested.remove(&(page_idx, kind, generation));

                    if kind == RenderKind::Page
                        && generation != self.generation.load(Ordering::Relaxed)
                    {
                        continue;
                    }

                    let name = match kind {
                        RenderKind::Page => "pdf_page",
                        RenderKind::Thumbnail => "pdf_thumbnail",
                    };
                    let texture = self
                        .ctx
                        .load_texture(name, image, egui::TextureOptions::LINEAR);
                    match kind {
                        RenderKind::Page => {
                            self.page_cache
                                .insert(page_idx, CachedPage { texture, generation });
                        }
                        RenderKind::Thumbnail => {
                            self.thumbnail_cache.insert(page_idx, texture);
                        }
                    }
                }
            }
        }
    }

    fn enqueue_page(&mut self, idx: usize) {
        let gen = self.generation.load(Ordering::Relaxed);
        let key = (idx, RenderKind::Page, gen);
        if !self.requested.insert(key) {
            return;
        }
        let scale = self.scale * self.ctx.pixels_per_point();
        let _ = self.request_tx.send(WorkerRequest::Render {
            page_idx: idx,
            kind: RenderKind::Page,
            scale,
            generation: gen,
        });
    }

    fn enqueue_thumbnail(&mut self, idx: usize) {
        let key = (idx, RenderKind::Thumbnail, 0);
        if !self.requested.insert(key) {
            return;
        }
        let _ = self.request_tx.send(WorkerRequest::Render {
            page_idx: idx,
            kind: RenderKind::Thumbnail,
            scale: THUMBNAIL_SCALE,
            generation: 0,
        });
    }

    fn handle_keys(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            ui.scroll_with_delta(Vec2 { x: 0., y: -20. });
        }

        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            ui.scroll_with_delta(Vec2 { x: 0., y: 20. });
        }

        if (ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::PageDown))
            || ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowRight)))
            && self.current_page != self.page_bounds.len() - 1
        {
            self.scroll_to = Some(self.current_page + 1);
        }

        if (ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::PageUp))
            || ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowLeft)))
            && self.current_page != 0
        {
            self.scroll_to = Some(self.current_page - 1);
        }

        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::End)) {
            self.scroll_to = Some(self.page_bounds.len() - 1);
        }

        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::Home)) {
            self.scroll_to = Some(0);
        }

        let event = ui.input(|r| {
            for e in &r.events {
                if let Event::Zoom(f) = e {
                    return Some(Event::Zoom(*f));
                }
            }
            None
        });

        let pos = ui.input(|r| r.pointer.latest_pos());
        if let Some(Event::Zoom(f)) = event {
            self.fit_height = false;
            self.fit_width = false;
            self.scale_updated(self.scale * f, pos);
        }
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui, band: Rect) {
        let t = ui.ctx().get_lb_theme();

        let sidebar_is_visible = match &mut self.sidebar {
            Some(s) => s.is_visible,
            None => false,
        };

        // Icon wash is the inner slot, not the full strip — same air as titleband.
        let hit = control_height();
        let ground = t.neutral_bg();
        // Full ink — muted idle on canvas reads as missing.
        let zoom_w = hit * 5.0 + 56.0;
        // Sidebar lives in the body below this strip; still center zoom over
        // the page column so the cluster sits above the document, not the rail.
        let page_right =
            if sidebar_is_visible { band.right() - SIDEBAR_WIDTH } else { band.right() };

        let zoom_left = band.left() + ((page_right - band.left() - zoom_w) / 2.0).max(0.0);
        let centered_rect = Rect::from_min_size(
            egui::pos2(zoom_left, band.top()),
            egui::vec2(zoom_w, band.height()),
        );

        if !self.compact && self.sidebar.is_some() {
            let toggle_r = Rect::from_center_size(
                egui::pos2(band.right() - 8.0 - hit / 2.0, band.center().y),
                Vec2::splat(hit),
            );
            crate::style::place_at(
                ui,
                toggle_r,
                egui::Layout::left_to_right(Align::Center),
                |ui| {
                    let tip =
                        if sidebar_is_visible { "Hide thumbnails" } else { "Show thumbnails" };
                    if toolbar_icon(ui, &t, phosphor::SIDEBAR_SIMPLE, ground, hit, tip) {
                        if let Some(sidebar) = &mut self.sidebar {
                            sidebar.is_visible = !sidebar.is_visible;
                        }
                    }
                },
            );
        }

        crate::style::place_at(
            ui,
            centered_rect,
            egui::Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if toolbar_icon(ui, &t, phosphor::MAGNIFYING_GLASS_MINUS, ground, hit, "Zoom out") {
                    self.scale_updated(self.scale - ZOOM_STOP, None);
                    self.fit_height = false;
                    self.fit_width = false;
                }

                let zoom_percentage = (self.scale * 100.).round();
                let pct = format!("{zoom_percentage:.0}%");
                let g = ui.painter().layout_no_wrap(
                    pct,
                    crate::style::TypeRole::Body.font_id(),
                    t.neutral_fg(),
                );
                let pct_w = g.size().x.max(48.0);
                let (pr, resp) = ui.allocate_exact_size(egui::vec2(pct_w, hit), sense_click());
                let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(&t));
                if fill != ground {
                    ui.painter().rect_filled(pr, Radius::Sm.corner(), fill);
                }
                ui.painter().galley(
                    egui::pos2(pr.center().x - g.size().x / 2.0, pr.center().y - g.size().y / 2.0),
                    g,
                    t.neutral_fg(),
                );
                tip_text(ui.ctx(), &resp, "Reset zoom");
                if resp.clicked() {
                    self.fit_width = false;
                    self.fit_height = false;
                    if (self.scale - 1.0).abs() > f32::EPSILON {
                        self.scale_updated(1.0, None);
                    }
                }

                if toolbar_icon(ui, &t, phosphor::MAGNIFYING_GLASS_PLUS, ground, hit, "Zoom in") {
                    self.scale_updated(ZOOM_STOP + self.scale, None);
                    self.fit_height = false;
                    self.fit_width = false;
                }
                if toolbar_icon(ui, &t, phosphor::ARROWS_HORIZONTAL, ground, hit, "Fit width") {
                    // Apply, don't toggle. Off is zoom in/out or reset.
                    self.fit_width = true;
                    self.fit_height = false;
                }
                if toolbar_icon(ui, &t, phosphor::ARROWS_VERTICAL, ground, hit, "Fit height") {
                    self.fit_height = true;
                    self.fit_width = false;
                }
            },
        );
    }

    fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        let (is_visible, thumbnails) = match &self.sidebar {
            Some(s) => (s.is_visible, s.thumbnails.clone()),
            None => return,
        };

        let sidebar_margin = 50.0;

        SidePanel::right("pdf_sidebar")
            .resizable(false)
            .show_separator_line(false)
            .show_animated_inside(ui, is_visible, |ui| {
                with_overlay_scroll(ui, Id::new("pdf_sidebar_overlay"), |ui| {
                    let out = ScrollArea::vertical()
                        .id_salt("pdf_sidebar")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Frame::default()
                                .inner_margin(sidebar_margin)
                                .show(ui, |ui| {
                                    for (i, p) in thumbnails.iter().enumerate() {
                                        let tint_color = if i == self.current_page {
                                            egui::Color32::WHITE
                                        } else {
                                            egui::Color32::GRAY.linear_multiply(0.5)
                                        };

                                        let (rect, res) =
                                            ui.allocate_exact_size(p.size, egui::Sense::click());

                                        if ui.is_rect_visible(rect) {
                                            let texture = self.get_thumbnail(i);
                                            egui::Image::new(egui::ImageSource::Texture(
                                                SizedTexture::new(&texture, p.size),
                                            ))
                                            .tint(tint_color)
                                            .paint_at(ui, rect);
                                        }

                                        if res.hovered() {
                                            ui.output_mut(|w| {
                                                w.cursor_icon = egui::CursorIcon::PointingHand
                                            })
                                        }
                                        if res.clicked() {
                                            self.scroll_to = Some(i);
                                        }

                                        if let Some(sb) = &mut self.sidebar {
                                            if i == self.current_page && sb.scroll_target != i {
                                                ui.scroll_to_rect(rect, None);
                                                sb.scroll_target = i;
                                            }
                                        }

                                        ui.add_space(sidebar_margin);
                                    }
                                });
                        });
                    (out.inner, out.state.offset.y, out.id)
                })
            });
    }

    fn show_pages(&mut self, ui: &mut Ui) {
        if self.page_bounds.is_empty() {
            CentralPanel::default().show_inside(ui, |ui| {
                if self.parse_failed {
                    let t = ui.ctx().get_lb_theme();
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            TypeRole::Body
                                .rich("Failed to load PDF")
                                .color(t.neutral_fg_secondary()),
                        );
                    });
                } else {
                    let age = self.opened.elapsed().as_secs_f32();
                    if age >= LOAD_SPINNER_DELAY {
                        loading_indicator(ui);
                    } else {
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_secs_f32(
                                (LOAD_SPINNER_DELAY - age).max(1.0 / 60.0),
                            ));
                    }
                }
            });
            return;
        }

        CentralPanel::default().show_inside(ui, |ui| {
            let panel = ui.available_rect_before_wrap();
            self.render_area = panel;
            // Fit leaves a little air so the page doesn't sit under the overlay bar.
            let fit_w = panel.width() * 0.95;
            let fit_h = panel.height() * 0.95;
            with_overlay_scroll(ui, Id::new("pdf_pages_overlay"), |ui| {
                let out = ScrollArea::both()
                    .id_salt("pdf_pages")
                    .animated(false)
                    .auto_shrink([false, false])
                    .show_viewport(ui, |ui, viewport| {
                        self.current_viewport = viewport;

                        let target_scale = if self.fit_width {
                            match self.page_dimensions.first().map(|d| d.0) {
                                Some(width) => fit_w / width,
                                None => 1.,
                            }
                        } else if self.fit_height {
                            match self.page_dimensions.first().map(|d| d.1) {
                                Some(height) => fit_h / height,
                                None => 1.,
                            }
                        } else {
                            self.scale
                        };

                        if target_scale != self.scale {
                            self.scale_updated(target_scale, None);
                        }

                        let max_height = self.page_bounds[self.page_bounds.len() - 1].max.y;
                        let max_width = self
                            .page_bounds
                            .iter()
                            .map(|r| r.width().ceil() as u32)
                            .max()
                            .unwrap_or_default() as f32;

                        // Floor at the panel width so the overlay bar sits on the
                        // viewport edge, not the (narrower) page's right.
                        let (rect, page_resp) = ui.allocate_exact_size(
                            egui::Vec2 { x: max_width.max(panel.width()), y: max_height },
                            egui::Sense::click(),
                        );
                        let t = ui.ctx().get_lb_theme();
                        if let Some(cmd) = crate::style::context_menu::show(&page_resp, &t, |e| {
                            e.item(phosphor::MAGNIFYING_GLASS_MINUS, "Zoom out", PdfCmd::ZoomOut);
                            e.item(phosphor::MAGNIFYING_GLASS_PLUS, "Zoom in", PdfCmd::ZoomIn);
                            e.item(phosphor::ARROWS_CLOCKWISE, "Reset zoom", PdfCmd::Reset);
                            e.separator();
                            e.item(phosphor::ARROWS_HORIZONTAL, "Fit width", PdfCmd::FitWidth);
                            e.item(phosphor::ARROWS_VERTICAL, "Fit height", PdfCmd::FitHeight);
                            if !self.compact && self.sidebar.is_some() {
                                e.separator();
                                let label = if self.sidebar.as_ref().is_some_and(|s| s.is_visible) {
                                    "Hide thumbnails"
                                } else {
                                    "Show thumbnails"
                                };
                                e.item(phosphor::SIDEBAR_SIMPLE, label, PdfCmd::ToggleSidebar);
                            }
                        }) {
                            match cmd {
                                PdfCmd::ZoomOut => {
                                    self.scale_updated(self.scale - ZOOM_STOP, None);
                                    self.fit_width = false;
                                    self.fit_height = false;
                                }
                                PdfCmd::ZoomIn => {
                                    self.scale_updated(ZOOM_STOP + self.scale, None);
                                    self.fit_width = false;
                                    self.fit_height = false;
                                }
                                PdfCmd::Reset => {
                                    self.fit_width = false;
                                    self.fit_height = false;
                                    if (self.scale - 1.0).abs() > f32::EPSILON {
                                        self.scale_updated(1.0, None);
                                    }
                                }
                                PdfCmd::FitWidth => {
                                    self.fit_width = true;
                                    self.fit_height = false;
                                }
                                PdfCmd::FitHeight => {
                                    self.fit_height = true;
                                    self.fit_width = false;
                                }
                                PdfCmd::ToggleSidebar => {
                                    if let Some(sidebar) = &mut self.sidebar {
                                        sidebar.is_visible = !sidebar.is_visible;
                                    }
                                }
                            }
                        }

                        let mut intersect_areas = vec![];
                        let draw_adjustment = rect.min.to_vec2();

                        for idx in 0..self.page_bounds.len() {
                            let page_rect = self.page_bounds[idx];
                            let center_adjustment = Vec2 {
                                x: if page_rect.width() < panel.width() {
                                    (panel.width() - page_rect.width()) / 2.
                                } else {
                                    0.
                                },
                                y: 0.,
                            };
                            let page_rect = page_rect.translate(center_adjustment);

                            if page_rect.intersects(viewport) {
                                let paint_location = page_rect.translate(draw_adjustment);
                                let img = self.get_page(idx);
                                img.paint_at(ui, paint_location);

                                intersect_areas
                                    .push((idx, page_rect.intersect(viewport).area() as u32));
                            }
                        }
                        self.handle_keys(ui);
                        if let Some(scroll_adj) = self.viewport_adjustment {
                            ui.scroll_with_delta(scroll_adj);
                            self.viewport_adjustment = None;

                            ui.ctx().request_repaint();
                        }
                        if let Some(scroll_idx) = self.scroll_to {
                            // this doesn't take into account `center_adjustment` from above
                            // but it doesn't matter, as if center_adjustment != 0, there is
                            // no horizontal scroll bar
                            ui.scroll_to_rect(
                                self.page_bounds[scroll_idx].translate(draw_adjustment),
                                Some(Align::TOP),
                            );
                            self.scroll_to = None;
                        }

                        let max_area = intersect_areas
                            .iter()
                            .map(|t| t.1)
                            .max()
                            .unwrap_or_default();
                        self.current_page = intersect_areas
                            .iter()
                            .filter(|t| t.1 == max_area)
                            .min_by_key(|t| t.0)
                            .map(|t| t.0)
                            .unwrap_or_default();
                    });
                (out.inner, out.state.offset.x + out.state.offset.y, out.id)
            });
        });
    }

    fn get_page(&mut self, idx: usize) -> Image<'_> {
        let texture = if idx < self.page_dimensions.len() {
            match self.page_cache.get(&idx).cloned() {
                Some(cached) => {
                    if cached.generation != self.generation.load(Ordering::Relaxed) {
                        self.enqueue_page(idx);
                    }
                    cached.texture
                }
                None => {
                    self.enqueue_page(idx);
                    self.placeholder.clone()
                }
            }
        } else {
            self.placeholder.clone()
        };

        Image::new(ImageSource::Texture(SizedTexture {
            id: texture.id(),
            size: texture.size_vec2(),
        }))
    }

    fn get_thumbnail(&mut self, idx: usize) -> TextureHandle {
        match self.thumbnail_cache.get(&idx).cloned() {
            Some(t) => t,
            None => {
                self.enqueue_thumbnail(idx);
                self.placeholder.clone()
            }
        }
    }

    fn compute_bounds(&mut self) {
        let mut pages = vec![];

        let mut offset = Pos2::ZERO;

        for &(w, h) in self.page_dimensions.iter() {
            let mut dims = Vec2::new(w, h);
            dims *= self.scale;

            pages.push(Rect { min: offset, max: offset + dims });

            offset.y += dims.y + SPACE_BETWEEN_PAGES;
        }

        let mut safe_area = Vec2::new(500., 500.);
        safe_area *= self.scale;
        pages.push(Rect { min: offset, max: offset + safe_area });

        self.page_bounds = pages;
    }

    /// zoom from indicates the position of the cursor. If None it will zoom from the center
    fn scale_updated(&mut self, new_scale: f32, zoom_from: Option<Pos2>) {
        if self.viewport_adjustment.is_some() {
            return;
        }
        if self.page_bounds.is_empty() {
            self.scale = new_scale;
            return;
        }
        // location in the old viewport
        let old_viewport_location = match zoom_from {
            Some(mouse) => {
                ((mouse - self.render_area.min) + self.current_viewport.min.to_vec2()).to_pos2()
            }
            None => self.current_viewport.center(),
        };

        // normalized point location in page space
        let normalized_page_space = old_viewport_location.to_vec2()
            / self.page_bounds.last().map(|r| r.max.to_vec2()).unwrap();

        self.scale = new_scale;
        self.page_bounds.clear();
        let gen = self
            .generation
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        self.requested
            .retain(|(_, kind, g)| *kind == RenderKind::Thumbnail || *g == gen);
        self.compute_bounds();

        // calculate the new viewport
        let new_location =
            normalized_page_space * self.page_bounds.last().map(|r| r.max.to_vec2()).unwrap();
        if !self.fit_width && !self.fit_height {
            self.viewport_adjustment = Some(-1. * (new_location - old_viewport_location.to_vec2()));
        }
        self.ctx.request_repaint();
    }
}

fn spawn_worker(
    bytes: Arc<Vec<u8>>, request_rx: Receiver<WorkerRequest>, response_tx: Sender<WorkerResponse>,
    ctx: Context, current_generation: Arc<AtomicU64>,
) {
    thread::spawn(move || {
        let pdf = {
            let _span = tracing::trace_span!("Pdf::parse").entered();
            match Pdf::new(bytes) {
                Ok(p) => p,
                Err(_) => {
                    let _ = response_tx.send(WorkerResponse::ParseFailed);
                    ctx.request_repaint();
                    return;
                }
            }
        };

        let page_dimensions: Vec<(f32, f32)> =
            pdf.pages().iter().map(|p| p.render_dimensions()).collect();
        if response_tx
            .send(WorkerResponse::Parsed { page_dimensions })
            .is_err()
        {
            return;
        }
        ctx.request_repaint();

        while let Ok(req) = request_rx.recv() {
            match req {
                WorkerRequest::Render { page_idx, kind, scale, generation } => {
                    let _span = tracing::trace_span!("Pdf::render", page_idx).entered();
                    if kind == RenderKind::Page
                        && generation != current_generation.load(Ordering::Relaxed)
                    {
                        continue;
                    }

                    let pages = pdf.pages();
                    let page = match pages.get(page_idx) {
                        Some(p) => p,
                        None => continue,
                    };
                    let pixmap = hayro::render(
                        page,
                        &InterpreterSettings::default(),
                        &RenderSettings { x_scale: scale, y_scale: scale, ..Default::default() },
                    );
                    let image = ColorImage::from_rgba_premultiplied(
                        [pixmap.width() as _, pixmap.height() as _],
                        pixmap.data_as_u8_slice(),
                    );
                    if response_tx
                        .send(WorkerResponse::Rendered { page_idx, kind, generation, image })
                        .is_err()
                    {
                        return;
                    }
                    ctx.request_repaint();
                }
            }
        }
    });
}

#[cfg(test)]
mod layout_diag {
    use super::{SIDEBAR_WIDTH, pdf_bands};
    use crate::style::CHROME_BAND_H;
    use egui::{Rect, pos2, vec2};

    #[test]
    fn diagnose_pdf_toolbar_band() {
        let host = Rect::from_min_size(pos2(80.0, 40.0), vec2(1200.0, 760.0));
        let (toolbar, body) = pdf_bands(host);

        assert!(
            (toolbar.height() - CHROME_BAND_H).abs() < 0.01,
            "toolbar height {} != {CHROME_BAND_H}",
            toolbar.height()
        );
        assert!((toolbar.top() - host.top()).abs() < 0.01);
        assert!((toolbar.left() - host.left()).abs() < 0.01);
        assert!((toolbar.right() - host.right()).abs() < 0.01);
        assert!((toolbar.bottom() - body.top()).abs() < 0.01, "bands must share the split");
        assert!((body.bottom() - host.bottom()).abs() < 0.01);
        assert!(
            (toolbar.height() + body.height() - host.height()).abs() < 0.01,
            "bands must cover host"
        );
        assert!(body.height() > CHROME_BAND_H, "pages must keep a real viewport under the strip");

        // CentralPanel leftover after eating the host is a 0-height sliver at
        // the bottom — that is not the toolbar.
        let leftover = Rect::from_min_max(pos2(host.left(), host.bottom()), host.max);
        assert!(leftover.height() < 0.01);
        assert!(
            toolbar.bottom() < leftover.top() - 1.0,
            "toolbar must not live in the leftover sliver"
        );
        assert!(toolbar.width() > SIDEBAR_WIDTH);
    }

    /// Tab content is nested in `centered_and_justified`. Claiming the strip
    /// first must leave a real page viewport, not a 0-height leftover sliver.
    #[test]
    fn reserved_toolbar_leaves_page_viewport() {
        let ctx = egui::Context::default();
        let mut leftover_h = 0.0_f32;
        let mut leftover_top = 0.0_f32;
        let mut toolbar_bottom = 0.0_f32;
        let mut host_top = 0.0_f32;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                        let host = ui.available_rect_before_wrap();
                        host_top = host.top();
                        let (toolbar, _) = pdf_bands(host);
                        crate::style::claim(ui, toolbar);
                        toolbar_bottom = toolbar.bottom();
                        let leftover = ui.available_rect_before_wrap();
                        leftover_h = leftover.height();
                        leftover_top = leftover.top();
                    });
                });
            });
        });
        assert!(
            (leftover_top - toolbar_bottom).abs() < 1.0,
            "leftover.top={leftover_top} toolbar.bottom={toolbar_bottom}"
        );
        assert!(leftover_h > 500.0, "leftover h={leftover_h} is a sliver, not the page viewport");
        assert!(
            (toolbar_bottom - host_top - CHROME_BAND_H).abs() < 1.0,
            "strip must sit at the top of the host"
        );
    }
}
