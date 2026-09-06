use std::ops::Deref as _;

use egui::{self, Color32, Pos2, Rect, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;
use resvg::usvg::Transform;
use tracing::error;

use crate::style::{
    Button, CHROME_BAND_GLYPH, Space, ThemeExt, TypeRole, canvas_overlay_frame, icon_button_circle,
    island, loading_indicator, phosphor, quiet_canvas_fills, sense_click, tip_text,
};
use crate::tab::input_controller::{
    InputController, InputControllerConfig, InputControllerEvent, LayoutContext,
};
use crate::tab::{ContextMenuTarget, ExtendedInput as _, ExtendedOutput as _};
use crate::widgets::image_cache::{ImageCache, ImageState};

const MIN_ZOOM_LEVEL: f32 = 0.1;
const ZOOM_STEP: f32 = 10.0;
const VIEWPORT_ISLAND_FALLBACK_WIDTH: f32 = 136.0;
const ZOOM_STOPS_POPOVER_WIDTH: f32 = 80.0;
const BRING_BACK_FALLBACK_WIDTH: f32 = 220.0;
const SCREEN_PADDING: egui::Pos2 =
    if cfg!(target_os = "android") { egui::pos2(10.0, 50.0) } else { egui::pos2(20.0, 20.0) };

pub struct ImageViewer {
    pub id: Uuid,
    images: ImageCache,
    input_controller: InputController,
    master_transform: Transform,
    viewport_popover: Option<ImageViewportPopover>,
    viewport_island: Option<egui::Rect>,
    zoom_pct_btn: Option<egui::Rect>,
    zoom_stops_popover: Option<egui::Rect>,
    bring_back_btn: Option<egui::Rect>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ImageViewportPopover {
    ZoomStops,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ImageCmd {
    Copy,
    Fit,
    ZoomIn,
    ZoomOut,
}

impl ImageViewer {
    pub fn new(id: Uuid, images: ImageCache) -> Self {
        Self {
            id,
            images,
            input_controller: InputController::new(InputControllerConfig::new(true, true)),
            master_transform: Transform::identity(),
            viewport_popover: None,
            viewport_island: None,
            zoom_pct_btn: None,
            zoom_stops_popover: None,
            bring_back_btn: None,
        }
    }

    fn url(&self) -> String {
        format!("lb://{}", self.id)
    }

    /// Start/pin decode without painting chrome. Search preview warms a pending
    /// image then promotes only once [`Self::paint_ready`].
    pub fn warm(&self) {
        let _ = self.images.get_or_load(&self.url(), self.id, false);
    }

    /// Texture settled (`Loaded` or `Failed`) — safe to mount the viewer.
    pub fn paint_ready(&self) -> bool {
        let state = self.images.get_or_load(&self.url(), self.id, false);
        let image_state = state.lock().unwrap().deref().clone();
        !matches!(image_state, ImageState::Loading)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Workspace `visuals::apply` restores default spacing / 17pt. DS chrome
        // assumes 0 item_spacing (same as search / desktop context).
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let t = ui.ctx().get_lb_theme();
        let mut painter = ui.painter().clone();
        let available = ui.available_rect_before_wrap();
        painter.set_clip_rect(available);
        painter.rect_filled(painter.clip_rect(), 0., t.neutral_bg());

        self.process_events(ui, available);

        let url = format!("lb://{}", self.id);
        let state = self.images.get_or_load(&url, self.id, true);
        let image_state = state.lock().unwrap().deref().clone();

        match image_state {
            ImageState::Loading => {
                loading_indicator(ui);
            }
            ImageState::Loaded(texture_id) => {
                let [img_w, img_h] = ui.ctx().tex_manager().read().meta(texture_id).unwrap().size;
                let image_size = Vec2::new(img_w as f32, img_h as f32);

                let rect = self.image_rect(available, image_size);

                painter.add(RectShape::filled(rect, 0.0, Color32::WHITE).with_texture(
                    texture_id,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                ));

                self.show_image_context_menu(ui, available);
                self.show_viewport_controls(ui, available, rect);
            }
            ImageState::Failed(ref msg) => {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        TypeRole::Body
                            .rich(format!("Failed to load image: {msg}"))
                            .color(t.neutral_fg_secondary()),
                    );
                });
            }
        }

        ui.ctx().pop_events();
    }

    pub fn detect_islands_interaction(&self, pos: Pos2) -> bool {
        [self.viewport_island, self.zoom_stops_popover, self.bring_back_btn]
            .into_iter()
            .flatten()
            .any(|rect| rect.contains(pos))
    }

    fn process_events(&mut self, ui: &mut egui::Ui, available: Rect) {
        let overlay_areas = [self.viewport_island, self.bring_back_btn, self.zoom_stops_popover]
            .into_iter()
            .flatten()
            .collect();
        let layout = LayoutContext::new(available, overlay_areas);
        for event in self.input_controller.process(ui, &layout) {
            if let InputControllerEvent::ViewportChange(transform) = event {
                self.transform_viewport(transform);
            }
        }
    }

    fn image_rect(&self, available: Rect, image_size: Vec2) -> Rect {
        let scale = (available.width() / image_size.x)
            .min(available.height() / image_size.y)
            .min(1.0);
        let base_rect = Rect::from_center_size(available.center(), image_size * scale);
        transform_rect(base_rect, self.master_transform)
    }

    fn transform_viewport(&mut self, transform: Transform) {
        let next_transform = self.master_transform.post_concat(transform);

        if next_transform.sx == 0.0 || next_transform.sy == 0.0 {
            return;
        }

        if self.master_transform.sx < MIN_ZOOM_LEVEL && next_transform.sx < self.master_transform.sx
        {
            return;
        }

        self.master_transform = next_transform;
    }

    fn show_viewport_controls(&mut self, ui: &mut egui::Ui, available: Rect, image_rect: Rect) {
        let t = ui.ctx().get_lb_theme();
        let viewport_island_width = self
            .viewport_island
            .map(|rect| rect.width())
            .unwrap_or(VIEWPORT_ISLAND_FALLBACK_WIDTH)
            .max(VIEWPORT_ISLAND_FALLBACK_WIDTH);
        let origin =
            egui::pos2(available.left() + SCREEN_PADDING.x, available.top() + SCREEN_PADDING.y);
        let viewport_rect =
            Rect::from_min_size(origin, egui::vec2(viewport_island_width, island::height() + 2.0));

        let island_res = ui
            .scope_builder(egui::UiBuilder::new().max_rect(viewport_rect), |ui| {
                island::frame(&t).show(ui, |ui| self.show_inner_viewport_island(ui, available, &t))
            })
            .inner
            .response;

        self.viewport_island = Some(island_res.rect);

        self.show_popovers(ui, available, island_res.rect);

        if let Some(res) = self.show_bring_back_btn(ui, available, image_rect, island_res.rect) {
            self.bring_back_btn = Some(res.rect);
        } else {
            self.bring_back_btn = None;
        }
    }

    fn show_inner_viewport_island(
        &mut self, ui: &mut egui::Ui, available: Rect, t: &crate::style::Theme,
    ) {
        ui.horizontal(|ui| {
            let zoom_percentage = (self.master_transform.sx * 100.0).round();
            let hit = island::icon_hit();
            let ground = island::ground(t);

            let minus = icon_button_circle(
                ui,
                t,
                phosphor::MAGNIFYING_GLASS_MINUS,
                true,
                ground,
                hit,
                CHROME_BAND_GLYPH,
            );
            tip_text(ui.ctx(), &minus, "Zoom out");
            if minus.clicked() && zoom_percentage > ZOOM_STEP {
                let target_zoom_percentage =
                    ((zoom_percentage / ZOOM_STEP).floor() - 1.0) * ZOOM_STEP;
                self.zoom_to(target_zoom_percentage, available.center());
            }

            let zoom_percentage_label = if self.master_transform.sx <= MIN_ZOOM_LEVEL {
                "MAX".to_string()
            } else {
                format!("{}%", zoom_percentage as i32)
            };

            let zoom_pct_btn = Button::secondary(t, zoom_percentage_label)
                .height(hit)
                .show(ui);
            self.zoom_pct_btn = Some(zoom_pct_btn.rect);
            tip_text(ui.ctx(), &zoom_pct_btn, "Zoom");

            if zoom_pct_btn.clicked() || zoom_pct_btn.drag_started() {
                self.toggle_viewport_popover(Some(ImageViewportPopover::ZoomStops));
            }

            let plus = icon_button_circle(
                ui,
                t,
                phosphor::MAGNIFYING_GLASS_PLUS,
                true,
                ground,
                hit,
                CHROME_BAND_GLYPH,
            );
            tip_text(ui.ctx(), &plus, "Zoom in");
            if plus.clicked() {
                let target_zoom_percentage =
                    ((zoom_percentage / ZOOM_STEP).floor() + 1.0) * ZOOM_STEP;
                self.zoom_to(target_zoom_percentage, available.center());
            }

            ui.add_space((50.0 - zoom_pct_btn.rect.width()).max(0.0));
        });
    }

    fn show_image_context_menu(&mut self, ui: &mut egui::Ui, available: Rect) {
        let response =
            ui.interact(available, ui.id().with("image_viewer_image"), egui::Sense::click());

        if cfg!(target_os = "ios") {
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    ui.ctx().set_context_menu(pos, ContextMenuTarget::Image);
                }
            }
            return;
        }

        let t = ui.ctx().get_lb_theme();
        if let Some(cmd) = crate::style::context_menu::show(&response, &t, |e| {
            e.item(phosphor::COPY, "Copy image", ImageCmd::Copy);
            e.separator();
            e.item(phosphor::ARROWS_OUT_SIMPLE, "Fit", ImageCmd::Fit);
            e.item(phosphor::MAGNIFYING_GLASS_PLUS, "Zoom in", ImageCmd::ZoomIn);
            e.item(phosphor::MAGNIFYING_GLASS_MINUS, "Zoom out", ImageCmd::ZoomOut);
        }) {
            match cmd {
                ImageCmd::Copy => self.copy_image(ui.ctx()),
                ImageCmd::Fit => self.reset_viewport(),
                ImageCmd::ZoomIn => {
                    let zoom_percentage = (self.master_transform.sx * 100.0).round();
                    let target = ((zoom_percentage / ZOOM_STEP).floor() + 1.0) * ZOOM_STEP;
                    self.zoom_to(target, available.center());
                }
                ImageCmd::ZoomOut => {
                    let zoom_percentage = (self.master_transform.sx * 100.0).round();
                    if zoom_percentage > ZOOM_STEP {
                        let target = ((zoom_percentage / ZOOM_STEP).floor() - 1.0) * ZOOM_STEP;
                        self.zoom_to(target, available.center());
                    }
                }
            }
        }
    }

    pub fn copy_image(&self, ctx: &egui::Context) {
        match self.images.color_image(self.id) {
            Ok(image) => ctx.copy_image(image),
            Err(err) => error!("failed to copy image to clipboard: {err}"),
        }
    }

    fn show_popovers(&mut self, ui: &mut egui::Ui, available: Rect, viewport_island_rect: Rect) {
        if let Some(ImageViewportPopover::ZoomStops) = self.viewport_popover {
            let t = ui.ctx().get_lb_theme();
            let popover_rect = {
                let x_center = self.zoom_pct_btn.unwrap_or(viewport_island_rect).center().x;
                let min = egui::pos2(
                    x_center - ZOOM_STOPS_POPOVER_WIDTH / 2.0,
                    viewport_island_rect.bottom() + Space::Sm.pts(),
                );
                // Ceiling only — Frame hugs content. A tall slot under the
                // workspace's justified layout stretched the first row (Fit).
                Rect::from_min_size(min, egui::vec2(ZOOM_STOPS_POPOVER_WIDTH, 400.0))
            };

            let (popover_res, _) = crate::style::place_at(
                ui,
                popover_rect,
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.set_max_width(ZOOM_STOPS_POPOVER_WIDTH);
                    canvas_overlay_frame(&t, Space::Xs)
                        .show(ui, |ui| self.show_zoom_stops_popover(ui, available, &t))
                        .response
                },
            );

            self.zoom_stops_popover = Some(popover_res.rect);
        } else {
            self.zoom_stops_popover = None;
        }
    }

    fn show_zoom_stops_popover(
        &mut self, ui: &mut egui::Ui, available: Rect, t: &crate::style::Theme,
    ) {
        let inner_w = (ZOOM_STOPS_POPOVER_WIDTH - Space::Xs.pts() * 2.0).max(1.0);
        ui.set_min_width(inner_w);
        ui.set_max_width(inner_w);
        let row_h = island::icon_hit();

        if zoom_stop_row(ui, t, "Fit", inner_w, row_h) {
            self.reset_viewport();
            self.viewport_popover = None;
        }

        for zoom_percentage in [120.0, 100.0, 80.0] {
            if zoom_stop_row(ui, t, &format!("{}%", zoom_percentage as i32), inner_w, row_h) {
                self.zoom_to(zoom_percentage, available.center());
                self.viewport_popover = None;
            }
        }
    }

    fn toggle_viewport_popover(&mut self, new_popover: Option<ImageViewportPopover>) {
        if self.viewport_popover == new_popover {
            self.viewport_popover = None;
        } else {
            self.viewport_popover = new_popover;
        }
    }

    fn show_bring_back_btn(
        &mut self, ui: &mut egui::Ui, available: Rect, image_rect: Rect, viewport_island_rect: Rect,
    ) -> Option<egui::Response> {
        if available.contains_rect(image_rect) || available.intersects(image_rect) {
            return None;
        }

        let bring_home_x_start = viewport_island_rect.right() + 15.0;
        let bring_home_y_start = viewport_island_rect.top();
        let bring_back_width = self
            .bring_back_btn
            .map(|rect| rect.width())
            .unwrap_or(BRING_BACK_FALLBACK_WIDTH)
            .max(BRING_BACK_FALLBACK_WIDTH);
        let bring_home_rect = Rect {
            min: egui::pos2(bring_home_x_start, bring_home_y_start),
            max: egui::Pos2 {
                x: bring_home_x_start + bring_back_width,
                y: viewport_island_rect.bottom(),
            },
        };

        let t = ui.ctx().get_lb_theme();
        let res = ui.scope_builder(egui::UiBuilder::new().max_rect(bring_home_rect), |ui| {
            island::frame(&t).show(ui, |ui| {
                if Button::secondary(&t, "Focus back to content")
                    .height(island::icon_hit())
                    .show(ui)
                    .clicked()
                {
                    self.reset_viewport();
                }
            })
        });

        Some(res.inner.response)
    }

    fn zoom_to(&mut self, zoom_percentage: f32, anchor: Pos2) {
        let zoom_delta = zoom_percentage / (self.master_transform.sx * 100.0);
        self.transform_viewport(
            Transform::identity()
                .post_scale(zoom_delta, zoom_delta)
                .post_translate((1.0 - zoom_delta) * anchor.x, (1.0 - zoom_delta) * anchor.y),
        );
    }

    fn reset_viewport(&mut self) {
        self.master_transform = Transform::identity();
    }
}

/// Full-width picker row: hover wash + hit span the menu, not the label.
fn zoom_stop_row(ui: &mut egui::Ui, t: &crate::style::Theme, label: &str, w: f32, h: f32) -> bool {
    use crate::style::chrome::row_wash_inset;
    use crate::style::{Radius, interact_fill_response};

    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), sense_click());
    let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
    if fill != t.neutral_bg() {
        ui.painter()
            .rect_filled(rect.shrink(row_wash_inset()), Radius::Sm.corner(), fill);
    }
    let g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), TypeRole::Body.font_id(), t.neutral_fg());
    let pad = Space::Xs.pts();
    ui.painter().galley(
        egui::pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0),
        g,
        t.neutral_fg(),
    );
    resp.clicked()
}

fn transform_point(point: Pos2, transform: Transform) -> Pos2 {
    Pos2 { x: transform.sx * point.x + transform.tx, y: transform.sy * point.y + transform.ty }
}

fn transform_rect(rect: Rect, transform: Transform) -> Rect {
    Rect { min: transform_point(rect.min, transform), max: transform_point(rect.max, transform) }
}

// a copy of this fn exists in Swift as isSupportedImageFormat()
pub fn is_supported_image_fmt(ext: &str) -> bool {
    let ext: &str = &ext.to_lowercase();

    // complete list derived from which features are enabled on image crate according to image-rs default features:
    // https://github.com/image-rs/image/blob/main/Cargo.toml#L70
    const IMG_FORMATS: [&str; 16] = [
        "avif", "bmp", "dds", "exr", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "png", "pnm", "qoi",
        "tga", "tiff", "webp",
    ];
    IMG_FORMATS.contains(&ext)
}
