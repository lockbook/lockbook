use std::ops::Deref as _;

use egui::{self, Color32, Pos2, Rect, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;
use resvg::usvg::Transform;

use crate::tab::ExtendedInput as _;
use crate::tab::input_controller::{
    InputController, InputControllerConfig, InputControllerEvent, LayoutContext,
};
use crate::theme::icons::Icon;
use crate::widgets::Button;
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

    pub fn show(&mut self, ui: &mut egui::Ui) {
        set_style(ui);

        let mut painter = ui.painter().clone();
        let available = ui.available_rect_before_wrap();
        painter.set_clip_rect(available);
        painter.rect_filled(painter.clip_rect(), 0., ui.visuals().extreme_bg_color);

        self.process_events(ui, available);

        let url = format!("lb://{}", self.id);
        let state = self.images.get_or_load(&url, self.id, true);
        let image_state = state.lock().unwrap().deref().clone();

        match image_state {
            ImageState::Loading => {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }
            ImageState::Loaded(texture_id) => {
                let [img_w, img_h] = ui.ctx().tex_manager().read().meta(texture_id).unwrap().size;
                let image_size = Vec2::new(img_w as f32, img_h as f32);

                let rect = self.image_rect(available, image_size);

                painter.add(RectShape::filled(rect, 0.0, Color32::WHITE).with_texture(
                    texture_id,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                ));

                self.show_viewport_controls(ui, available, rect);
            }
            ImageState::Failed(ref msg) => {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("Failed to load image: {msg}"));
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
        let viewport_island_width = self
            .viewport_island
            .map(|rect| rect.width())
            .unwrap_or(VIEWPORT_ISLAND_FALLBACK_WIDTH)
            .max(VIEWPORT_ISLAND_FALLBACK_WIDTH);
        let viewport_rect = Rect {
            min: egui::pos2(
                available.left() + SCREEN_PADDING.x,
                available.top() + SCREEN_PADDING.y,
            ),
            max: egui::Pos2 {
                x: available.left() + SCREEN_PADDING.x + viewport_island_width,
                y: available.top() + SCREEN_PADDING.y + 35.0,
            },
        };

        let island_res = ui
            .scope_builder(egui::UiBuilder::new().max_rect(viewport_rect), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| self.show_inner_viewport_island(ui, available))
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

    fn show_inner_viewport_island(&mut self, ui: &mut egui::Ui, available: Rect) {
        ui.horizontal(|ui| {
            let zoom_percentage = (self.master_transform.sx * 100.0).round();
            let size = 15.0;

            if ui
                .add_enabled_ui(zoom_percentage > ZOOM_STEP, |ui| {
                    Button::default().icon(&Icon::ZOOM_OUT.size(size)).show(ui)
                })
                .inner
                .clicked()
            {
                let target_zoom_percentage =
                    ((zoom_percentage / ZOOM_STEP).floor() - 1.0) * ZOOM_STEP;
                self.zoom_to(target_zoom_percentage, available.center());
            }

            let zoom_percentage_label = if self.master_transform.sx <= MIN_ZOOM_LEVEL {
                "MAX".to_string()
            } else {
                format!("{}%", zoom_percentage as i32)
            };

            let zoom_pct_btn = Button::default().text(zoom_percentage_label).show(ui);
            self.zoom_pct_btn = Some(zoom_pct_btn.rect);

            if zoom_pct_btn.clicked() || zoom_pct_btn.drag_started() {
                self.toggle_viewport_popover(Some(ImageViewportPopover::ZoomStops));
            }

            if Button::default()
                .icon(&Icon::ZOOM_IN.size(size))
                .show(ui)
                .clicked()
            {
                let target_zoom_percentage =
                    ((zoom_percentage / ZOOM_STEP).floor() + 1.0) * ZOOM_STEP;
                self.zoom_to(target_zoom_percentage, available.center());
            };

            ui.add_space((50.0 - zoom_pct_btn.rect.width()).max(0.0));
        });
    }

    fn show_popovers(&mut self, ui: &mut egui::Ui, available: Rect, viewport_island_rect: Rect) {
        if let Some(ImageViewportPopover::ZoomStops) = self.viewport_popover {
            ui.visuals_mut().window_corner_radius /= 2.0;

            let popover_rect = {
                let x_center = self.zoom_pct_btn.unwrap_or(viewport_island_rect).center().x;
                let min = egui::pos2(
                    x_center - ZOOM_STOPS_POPOVER_WIDTH / 2.0,
                    viewport_island_rect.bottom() + 10.0,
                );

                Rect { min, max: min + egui::vec2(ZOOM_STOPS_POPOVER_WIDTH, 0.0) }
            };

            let popover_res = ui
                .scope_builder(egui::UiBuilder::new().max_rect(popover_rect), |ui| {
                    egui::Frame::window(ui.style())
                        .show(ui, |ui| self.show_zoom_stops_popover(ui, available))
                })
                .inner
                .response;

            self.zoom_stops_popover = Some(popover_res.rect);
        } else {
            self.zoom_stops_popover = None;
        }
    }

    fn show_zoom_stops_popover(&mut self, ui: &mut egui::Ui, available: Rect) {
        ui.set_min_width(
            ZOOM_STOPS_POPOVER_WIDTH
                - ui.style().spacing.window_margin.left as f32
                - ui.style().spacing.window_margin.right as f32,
        );

        if Button::default().text("FIT").show(ui).clicked() {
            self.reset_viewport();
            self.viewport_popover = None;
        }

        for zoom_percentage in [120.0, 100.0, 80.0] {
            if Button::default()
                .text(format!("{}%", zoom_percentage as i32))
                .show(ui)
                .clicked()
            {
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

        let res = ui.scope_builder(egui::UiBuilder::new().max_rect(bring_home_rect), |ui| {
            egui::Frame::window(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let text_stroke = egui::Stroke {
                            color: ui.visuals().widgets.active.bg_fill,
                            ..Default::default()
                        };

                        ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
                        ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
                        ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;

                        if Button::default()
                            .text("Focus back to content")
                            .show(ui)
                            .clicked()
                        {
                            self.reset_viewport();
                        }
                    })
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

fn set_style(ui: &mut egui::Ui) {
    let toolbar_margin = egui::Margin::symmetric(15, 7);
    ui.visuals_mut().window_corner_radius = egui::CornerRadius::same(30);
    ui.style_mut().spacing.window_margin = toolbar_margin;
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::new(13.0, egui::FontFamily::Proportional));
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::new(13.0, egui::FontFamily::Proportional));

    ui.visuals_mut().widgets.active.bg_fill =
        ui.visuals_mut().widgets.active.bg_fill.linear_multiply(0.7);

    if ui.visuals().dark_mode {
        ui.visuals_mut().window_stroke =
            egui::Stroke::new(0.5, egui::Color32::from_rgb(56, 56, 56));
        ui.visuals_mut().window_fill = egui::Color32::from_rgb(30, 30, 30);
        ui.visuals_mut().window_shadow = egui::Shadow::NONE;
    } else {
        ui.visuals_mut().window_stroke =
            egui::Stroke::new(0.5, egui::Color32::from_rgb(235, 235, 235));
        ui.visuals_mut().window_shadow = egui::Shadow {
            offset: [1, 8],
            blur: 20,
            spread: 0,
            color: egui::Color32::from_black_alpha(10),
        };
        ui.visuals_mut().window_fill = ui.visuals().extreme_bg_color;
    }
}
