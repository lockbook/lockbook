use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use crate::style::chrome::{Radius, STROKE_HAIRLINE, phosphor, pin_shadow};
use crate::style::{
    Space, ThemeExt as _, TypeRole, claim, control_height, icon_button_glyph, island,
    loading_indicator, message_indicator, overlay_scroll, phosphor_ui_font_id, place_at,
    surface_motion, tip_text, with_overlay_scroll,
};
use egui::{
    Align, Align2, Color32, ColorImage, Context, CursorIcon, Event, Id, Image, ImageSource, Key,
    Layout, Modifiers, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2,
    load::SizedTexture, pos2, vec2,
};
use hayro::{InterpreterSettings, Pdf, RenderSettings};
use lb_rs::Uuid;

pub struct PdfViewer {
    pub id: Uuid,

    ctx: Context,

    page_dimensions: Vec<(f32, f32)>,

    parse_failed: bool,

    /// the bounds of all the pages, as influenced by scale.
    page_bounds: Vec<Rect>,

    page_cache: HashMap<usize, CachedPage>,

    thumbnail_cache: HashMap<usize, TextureHandle>,

    placeholder: TextureHandle,

    generation: Arc<AtomicU64>,

    requested: HashSet<(usize, RenderKind, Generation)>,

    request_tx: Sender<WorkerRequest>,
    response_rx: Receiver<WorkerResponse>,

    worker_busy: Arc<AtomicBool>,

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
    /// immediately and keep animations on
    viewport_adjustment: Option<Vec2>,

    sidebar: Option<SideBar>,

    /// Phone, iPad compact, or Android: no page sidebar.
    pub compact: bool,
}

struct SideBar {
    thumbnails: Vec<Content>,
    is_visible: bool,
    scroll_target: usize,
    offset: f32,
    /// Eases `offset` here to bring the current page into view.
    follow: Option<f32>,
    /// Points per second left from a touch fling.
    fling: f32,
    /// When the finger last moved; a finger held still before lifting doesn't fling.
    moved_at: f64,
    /// The thumbnail under the press; iOS reports no pointer on the release frame.
    pressed: Option<usize>,
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

enum WorkerRequest {
    Render { page_idx: usize, kind: RenderKind, scale: f32, generation: Generation },
}

enum WorkerResponse {
    Parsed { page_dimensions: Vec<(f32, f32)> },
    ParseFailed,
    Rendered { page_idx: usize, kind: RenderKind, generation: Generation, image: ColorImage },
}

const ZOOM_STOP: f32 = 0.1;
const SIDEBAR_WIDTH: f32 = 200.0;
const SCREEN_PADDING: Pos2 =
    if cfg!(target_os = "android") { egui::pos2(10.0, 10.0) } else { egui::pos2(20.0, 20.0) };
const SPACE_BETWEEN_PAGES: f32 = 10.0;
const THUMBNAIL_SCALE: f32 = 0.5;

impl PdfViewer {
    pub fn new(id: Uuid, bytes: Vec<u8>, ctx: &egui::Context) -> Self {
        let bytes: Arc<Vec<u8>> = Arc::new(bytes);
        let (request_tx, request_rx) = mpsc::channel::<WorkerRequest>();
        let (response_tx, response_rx) = mpsc::channel::<WorkerResponse>();
        let worker_busy = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(AtomicU64::new(0));
        spawn_worker(
            bytes,
            request_rx,
            response_tx,
            ctx.clone(),
            worker_busy.clone(),
            generation.clone(),
        );

        let placeholder = ctx.load_texture(
            "pdf_placeholder",
            ColorImage::from_rgba_premultiplied([1, 1], &[255, 255, 255, 255]),
            egui::TextureOptions::LINEAR,
        );

        Self {
            id,
            sidebar: Default::default(),
            compact: false,
            page_dimensions: Default::default(),
            parse_failed: false,
            page_cache: Default::default(),
            thumbnail_cache: Default::default(),
            placeholder,
            generation,
            requested: Default::default(),
            request_tx,
            response_rx,
            worker_busy,
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
        }
    }

    fn setup_sidebar(&mut self) {
        let inner_width = SIDEBAR_WIDTH - 2.0 * Space::Lg.pts();

        let thumbnails = self
            .page_dimensions
            .iter()
            .map(|&(w, h)| {
                let aspect = if w > 0. { h / w } else { 1. };
                Content { size: egui::vec2(inner_width, inner_width * aspect) }
            })
            .collect();

        self.sidebar = Some(SideBar {
            thumbnails,
            is_visible: true,
            scroll_target: 0,
            offset: 0.0,
            follow: None,
            fling: 0.0,
            moved_at: 0.0,
            pressed: None,
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.drain_responses();

        let t = ui.ctx().get_lb_theme();
        let host = ui.available_rect_before_wrap();
        ui.painter().rect_filled(host, 0., t.neutral_bg());

        // pages fit the width they'll have once the sidebar settles
        let mut pages = host;
        let mut fit_width = host.width();
        if let Some(sidebar) = self.sidebar.as_ref().filter(|_| !self.compact) {
            let id = Id::unique("pdf_sidebar_open").with(self.id);
            let shown = SIDEBAR_WIDTH * surface_motion(ui.ctx(), id, sidebar.is_visible).slide;
            if sidebar.is_visible {
                fit_width -= SIDEBAR_WIDTH;
            }
            if shown > 0.0 {
                pages.max.x -= shown;
                let full = Rect::from_min_size(
                    pos2(pages.max.x, host.min.y),
                    vec2(SIDEBAR_WIDTH, host.height()),
                );
                self.show_sidebar(ui, full, full.intersect(host));
            }
        }
        let controls = crate::safe_area::inset_rect(ui.ctx(), host).intersect(pages);
        self.show_pages(ui, pages, fit_width);
        self.show_toolbar(ui, controls);
        claim(ui, host);
    }

    fn drain_responses(&mut self) {
        while let Ok(resp) = self.response_rx.try_recv() {
            match resp {
                WorkerResponse::Parsed { page_dimensions } => {
                    self.page_dimensions = page_dimensions;
                    self.compute_bounds();
                    self.setup_sidebar();
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

    /// Zoom, fit and sidebar controls floating at the top right of the pages.
    fn show_toolbar(&mut self, ui: &mut Ui, controls: Rect) {
        let t = ui.ctx().get_lb_theme();
        let hit = control_height();
        let font = TypeRole::Body.font_id();
        let pct_w = ui
            .painter()
            .layout_no_wrap("0000%".into(), font.clone(), Color32::PLACEHOLDER)
            .size()
            .x;
        let sep_w = 2.0 * Space::Xs.pts() + STROKE_HAIRLINE;
        let has_sidebar = self.sidebar.is_some() && !self.compact;
        let icons = if has_sidebar { 5.0 } else { 4.0 };
        let seps = if has_sidebar { 2.0 } else { 1.0 };
        let width = island::width(icons * hit + pct_w + seps * sep_w);
        if controls.width() < width + 2.0 * SCREEN_PADDING.x
            || controls.height() < island::height() + 2.0 * SCREEN_PADDING.y
        {
            return;
        }
        let rect = Rect::from_min_size(
            pos2(controls.right() - SCREEN_PADDING.x - width, controls.top() + SCREEN_PADDING.y),
            vec2(width, island::height()),
        );

        if self.worker_busy.load(Ordering::Relaxed) {
            ui.ctx().request_repaint();
            let angle = (ui.input(|i| i.time) * std::f64::consts::TAU) as f32;
            let g = ui.painter().layout_no_wrap(
                phosphor::SPINNER_GAP.into(),
                phosphor_ui_font_id(),
                t.accent(),
            );
            let center = pos2(rect.left() - Space::Sm.pts() - g.size().x / 2.0, rect.center().y);
            ui.painter().add(
                egui::epaint::TextShape::new(center - g.size() / 2.0, g, t.accent())
                    .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
            );
        }

        let zoom = format!("{}%", (self.scale * 100.).round());
        let sidebar_open = self.sidebar.as_ref().is_some_and(|s| s.is_visible);
        let (fit_width, fit_height) = (self.fit_width, self.fit_height);
        let mut clicked = None;
        island::island(ui, rect, |ui| {
            let mut button = |ui: &mut Ui, icon, active, tip, cmd| {
                let r = icon_button_glyph(
                    ui,
                    &t,
                    icon,
                    active,
                    island::fill(&t),
                    hit,
                    TypeRole::Body.size(),
                );
                tip_text(ui.ctx(), &r, tip);
                if r.clicked() {
                    clicked = Some(cmd);
                }
            };
            let sep = |ui: &mut Ui| {
                let (r, _) = ui.allocate_exact_size(vec2(sep_w, hit), Sense::hover());
                ui.painter().vline(
                    r.center().x,
                    r.shrink2(vec2(0.0, Space::Xs.pts())).y_range(),
                    Stroke::new(STROKE_HAIRLINE, t.neutral()),
                );
            };

            button(ui, phosphor::MAGNIFYING_GLASS_MINUS, false, "Zoom out", PdfCmd::ZoomOut);
            let (r, _) = ui.allocate_exact_size(vec2(pct_w, hit), Sense::hover());
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                &zoom,
                font.clone(),
                t.neutral_fg_secondary(),
            );
            button(ui, phosphor::MAGNIFYING_GLASS_PLUS, false, "Zoom in", PdfCmd::ZoomIn);
            sep(ui);
            button(ui, phosphor::ARROWS_HORIZONTAL, fit_width, "Fit width", PdfCmd::FitWidth);
            button(ui, phosphor::ARROWS_VERTICAL, fit_height, "Fit height", PdfCmd::FitHeight);
            if has_sidebar {
                sep(ui);
                button(ui, phosphor::SIDEBAR_SIMPLE, sidebar_open, "Pages", PdfCmd::Sidebar);
            }
        });

        match clicked {
            Some(PdfCmd::ZoomOut) => {
                self.fit_height = false;
                self.fit_width = false;
                self.scale_updated((self.scale - ZOOM_STOP).max(ZOOM_STOP), None);
            }
            Some(PdfCmd::ZoomIn) => {
                self.fit_height = false;
                self.fit_width = false;
                self.scale_updated(ZOOM_STOP + self.scale, None);
            }
            Some(PdfCmd::FitWidth) => {
                self.fit_width = !self.fit_width;
                if self.fit_width {
                    self.fit_height = false;
                }
            }
            Some(PdfCmd::FitHeight) => {
                self.fit_height = !self.fit_height;
                if self.fit_height {
                    self.fit_width = false;
                }
            }
            Some(PdfCmd::Sidebar) => {
                if let Some(sidebar) = &mut self.sidebar {
                    sidebar.is_visible = !sidebar.is_visible;
                }
            }
            None => {}
        }
    }

    /// Page thumbnails in a column at the right, each with its page number;
    /// the current page is outlined. Keeps its own scroll offset.
    /// `rect` is the sidebar's full size; `clip` is the part slid into view.
    fn show_sidebar(&mut self, ui: &mut Ui, rect: Rect, clip: Rect) {
        let Some(sidebar) = &self.sidebar else { return };
        let t = ui.ctx().get_lb_theme();
        ui.painter().rect_filled(clip, 0., t.neutral_bg_secondary());

        // measure: each row is a thumbnail over its page number
        let pad = Space::Lg.pts();
        let label_h = Space::Xs.pts() + TypeRole::Body.line_height();
        let mut rows = Vec::with_capacity(sidebar.thumbnails.len());
        let mut y = pad;
        for thumb in &sidebar.thumbnails {
            rows.push(Rect::from_min_size(pos2(pad, y), thumb.size));
            y += thumb.size.y + label_h + Space::Md.pts();
        }
        let content_h = y - Space::Md.pts() + pad;
        let max_offset = (content_h - rect.height()).max(0.0);

        let id = Id::unique("pdf_sidebar").with(self.id);
        let resp = ui.interact(clip, id, Sense::click_and_drag());
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let touch = ui.input(|i| i.any_touches());
        let (mut offset, mut follow, mut fling) = (sidebar.offset, sidebar.follow, sidebar.fling);
        let mut moved_at = sidebar.moved_at;
        let now = ui.input(|i| i.time);

        let wheel = if ui.rect_contains_pointer(clip) {
            ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta).y)
        } else {
            0.0
        };
        let dragging = resp.dragged() && touch;
        if wheel != 0.0 || dragging {
            follow = None;
        }
        offset -= wheel;
        if dragging {
            // the release frame has no touch or pointer velocity, so track it here
            let dy = resp.drag_delta().y;
            offset -= dy;
            fling = 0.5 * fling - 0.5 * dy / dt.max(1e-3);
            if dy != 0.0 {
                moved_at = now;
            }
        } else if wheel != 0.0 || (resp.drag_stopped() && now - moved_at > 0.05) {
            fling = 0.0;
        } else if fling.abs() > 5.0 {
            offset += fling * dt;
            fling *= (-4.0 * dt).exp();
            ui.ctx().request_repaint();
        } else {
            fling = 0.0;
        }

        let current = self.current_page;
        if sidebar.scroll_target != current {
            if let Some(row) = rows.get(current) {
                let bottom = row.bottom() + label_h;
                if row.top() < offset {
                    follow = Some(row.top() - pad);
                } else if bottom > offset + rect.height() {
                    follow = Some(bottom + pad - rect.height());
                }
            }
        }
        if let Some(target) = follow {
            let target = target.clamp(0.0, max_offset);
            offset += (target - offset) * (1.0 - (-12.0 * dt).exp());
            if (target - offset).abs() < 0.5 {
                offset = target;
                follow = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
        let mut offset = offset.clamp(0.0, max_offset);
        if offset == 0.0 || offset == max_offset {
            fling = 0.0;
        }
        let origin = rect.min - vec2(0.0, offset);

        let row_at = |p: Pos2| {
            rows.iter().position(|r| {
                r.translate(origin.to_vec2())
                    .expand2(vec2(0.0, label_h))
                    .contains(p)
            })
        };
        let hovered = resp.hover_pos().and_then(row_at);
        if hovered.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let mut pressed = sidebar.pressed;
        if resp.is_pointer_button_down_on() {
            if let Some(p) = resp.interact_pointer_pos() {
                pressed = row_at(p);
            }
        }
        if resp.clicked() {
            if let Some(i) = pressed.or(hovered) {
                self.scroll_to = Some(i);
            }
        }
        if !resp.is_pointer_button_down_on() {
            pressed = None;
        }

        let painter = ui.painter_at(clip);
        let font = TypeRole::Body.font_id();
        for (i, row) in rows.iter().enumerate() {
            let r = row.translate(origin.to_vec2());
            if r.bottom() + label_h < rect.top() || r.top() > rect.bottom() {
                continue;
            }
            let texture = self.get_thumbnail(i);
            painter.image(
                texture.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            let ink = if i == current {
                painter.rect_stroke(
                    r,
                    Radius::Sm.corner(),
                    Stroke::new(2.0_f32, t.accent()),
                    StrokeKind::Outside,
                );
                t.accent()
            } else if hovered == Some(i) {
                t.neutral_fg()
            } else {
                t.neutral_fg_secondary()
            };
            painter.text(
                pos2(r.center().x, r.bottom() + Space::Xs.pts()),
                Align2::CENTER_TOP,
                (i + 1).to_string(),
                font.clone(),
                ink,
            );
        }

        if clip == rect {
            // following the current page is the program's scroll, not the user's
            let quiet = wheel == 0.0 && !dragging && fling == 0.0;
            overlay_scroll::scroller_bar(ui, id, rect, content_h, &mut offset, quiet);
        }

        if let Some(sb) = &mut self.sidebar {
            sb.offset = offset;
            sb.follow = follow;
            sb.fling = fling;
            sb.moved_at = moved_at;
            sb.pressed = pressed;
            sb.scroll_target = current;
        }
    }

    fn show_pages(&mut self, ui: &mut Ui, rect: Rect, fit_width: f32) {
        let top = pages_top_inset();
        let view_width = rect.width();
        let available_width = fit_width * 0.95;
        let available_height = (rect.height() - top) * 0.95;
        self.render_area = rect;

        if self.page_bounds.is_empty() {
            place_at(ui, rect, Layout::top_down(Align::Center), |ui| {
                if self.parse_failed {
                    message_indicator(ui, phosphor::WARNING_CIRCLE, "Failed to load PDF");
                } else if self.sidebar.is_some() {
                    // parsed, but nothing to show
                    message_indicator(ui, phosphor::FILE_PDF, "This PDF has no pages");
                } else {
                    loading_indicator(ui);
                }
            });
            return;
        }

        let t = ui.ctx().get_lb_theme();
        place_at(ui, rect, Layout::top_down(Align::Min), |ui| {
            with_overlay_scroll(ui, Id::unique("pdf_pages").with(self.id), |ui| {
                // a rescale's anchor lands as an absolute offset the frame after,
                // clamped against the new content size
                let mut area = ScrollArea::both().animated(false);
                if let Some(adj) = self.viewport_adjustment.take() {
                    area = area.scroll_offset(self.current_viewport.min.to_vec2() - adj);
                    ui.ctx().request_repaint();
                }
                let out = area.show_viewport(ui, |ui, viewport| {
                    self.current_viewport = viewport;

                    let target_scale = if self.fit_width {
                        match self.page_dimensions.first().map(|d| d.0) {
                            Some(width) => available_width / width,
                            None => 1.,
                        }
                    } else if self.fit_height {
                        match self.page_dimensions.first().map(|d| d.1) {
                            Some(height) => available_height / height,
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

                    let (_, rect) = ui
                        .allocate_space(egui::Vec2 { x: max_width.max(view_width), y: max_height });

                    let mut intersect_areas = vec![];
                    let draw_adjustment = rect.min.to_vec2();

                    for idx in 0..self.page_bounds.len() {
                        let page_rect = self.page_bounds[idx];
                        let center_adjustment = Vec2 {
                            x: if page_rect.width() < view_width {
                                (view_width - page_rect.width()) / 2.
                            } else {
                                0.
                            },
                            y: 0.,
                        };
                        let page_rect = page_rect.translate(center_adjustment);

                        if page_rect.intersects(viewport) {
                            let paint_location = page_rect.translate(draw_adjustment);
                            ui.painter()
                                .add(pin_shadow(&t).as_shape(paint_location, 0.));
                            let img = self.get_page(idx);
                            img.paint_at(ui, paint_location);

                            intersect_areas
                                .push((idx, page_rect.intersect(viewport).area() as u32));
                        }
                    }
                    self.handle_keys(ui);
                    if let Some(scroll_idx) = self.scroll_to {
                        // this doesn't take into account `center_adjustment` from above
                        // but it doesn't matter, as if center_adjustment != 0, there is
                        // no horizontal scroll bar
                        ui.scroll_to_rect(
                            self.page_bounds[scroll_idx]
                                .translate(draw_adjustment)
                                .expand2(vec2(0.0, top)),
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
                ((), out.state.clamped_offset().y, out.id)
            });
        });
    }

    fn get_page(&mut self, idx: usize) -> Image<'_> {
        let texture = if idx < self.page_bounds.len() {
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

        let mut offset = pos2(0.0, pages_top_inset());

        for &(w, h) in self.page_dimensions.iter() {
            let mut dims = Vec2::new(w, h);
            dims *= self.scale;

            pages.push(Rect { min: offset, max: offset + dims });

            offset.y += dims.y + SPACE_BETWEEN_PAGES;
        }

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
        println!("zoom from {zoom_from:?}");
        let old_viewport_location = match zoom_from {
            Some(mouse) => {
                ((mouse - self.render_area.min) + self.current_viewport.min.to_vec2()).to_pos2()
            }
            // fit modes keep the page at the top of the view where it was
            None if self.fit_width || self.fit_height => self.current_viewport.min,
            None => self.current_viewport.center(),
        };
        println!("old_viewport {old_viewport_location:?}");

        // normalized point location in page space
        let normalized_page_space = old_viewport_location.to_vec2()
            / self.page_bounds.last().map(|r| r.max.to_vec2()).unwrap();
        println!("normalized_page space {normalized_page_space:?}");

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
        println!("new location {new_location:?}");
        self.viewport_adjustment = Some(-1. * (new_location - old_viewport_location.to_vec2()));
        println!("adjustment {:?}", self.viewport_adjustment);
        self.ctx.request_repaint();
    }
}

/// Room above the first page so the toolbar doesn't cover it.
fn pages_top_inset() -> f32 {
    SCREEN_PADDING.y + island::height() + Space::Sm.pts()
}

#[derive(Clone, Copy)]
enum PdfCmd {
    ZoomOut,
    ZoomIn,
    FitWidth,
    FitHeight,
    Sidebar,
}

fn spawn_worker(
    bytes: Arc<Vec<u8>>, request_rx: Receiver<WorkerRequest>, response_tx: Sender<WorkerResponse>,
    ctx: Context, worker_busy: Arc<AtomicBool>, current_generation: Arc<AtomicU64>,
) {
    struct BusyGuard<'a>(&'a AtomicBool);
    impl Drop for BusyGuard<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Relaxed);
        }
    }

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
            worker_busy.store(true, Ordering::Relaxed);
            let _busy = BusyGuard(&worker_busy);

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
