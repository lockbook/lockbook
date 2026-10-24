use std::ops::Deref as _;

use egui::{Color32, Pos2, Rect, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;
use resvg::usvg::Transform;
use tracing::error;

use crate::style::chrome::phosphor;
use crate::style::space::control as control_space;
use crate::style::{
    Button, Space, ThemeExt as _, TypeRole, context_menu, control_height, icon_button_glyph,
    island, loading_indicator, message_indicator, tip_text,
};
use crate::tab::input_controller::{
    InputController, InputControllerConfig, InputControllerEvent, LayoutContext,
};
use crate::tab::{ContextMenuTarget, ExtendedInput as _, ExtendedOutput as _};
use crate::widgets::image_cache::{ImageCache, ImageKey, ImageState};

const MIN_ZOOM_LEVEL: f32 = 0.1;
const ZOOM_STEP: f32 = 10.0;
const SCREEN_PADDING: egui::Pos2 =
    if cfg!(target_os = "android") { egui::pos2(10.0, 50.0) } else { egui::pos2(20.0, 20.0) };
const BACK_LABEL: &str = "Back to image";

pub struct ImageViewer {
    pub id: Uuid,
    images: ImageCache,
    input_controller: InputController,
    master_transform: Transform,
    /// Last frame's island rects; the canvas ignores presses that start on them.
    islands: Vec<Rect>,
    /// A menu is open over the canvas; touches go to it.
    menu_open: bool,
}

#[derive(Clone, Copy)]
enum ZoomStop {
    Fit,
    Percent(f32),
}

#[derive(Clone, Copy)]
enum ImageCmd {
    Copy,
}

impl ImageViewer {
    pub fn new(id: Uuid, images: ImageCache) -> Self {
        Self {
            id,
            images,
            input_controller: InputController::new(InputControllerConfig::new(true, true)),
            master_transform: Transform::identity(),
            islands: Vec::new(),
            menu_open: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let t = ui.ctx().get_lb_theme();
        let mut painter = ui.painter().clone();
        let available = ui.available_rect_before_wrap();
        painter.set_clip_rect(available);
        painter.rect_filled(painter.clip_rect(), 0., t.neutral_bg());

        self.menu_open = context_menu::is_any_open(ui.ctx());
        if !self.menu_open {
            self.process_events(ui, available);
        }

        let state = self.images.get_or_load(&ImageKey::File(self.id), true);
        let image_state = state.lock().unwrap().deref().clone();

        self.islands.clear();
        match image_state {
            ImageState::Loading => loading_indicator(ui),
            ImageState::Loaded(texture_id) => {
                let [img_w, img_h] = ui.ctx().tex_manager().read().meta(texture_id).unwrap().size;
                let image_size = Vec2::new(img_w as f32, img_h as f32);

                let rect = self.image_rect(available, image_size);

                painter.add(RectShape::filled(rect, 0.0, Color32::WHITE).with_texture(
                    texture_id,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                ));

                self.show_image_context_menu(ui, available, rect);
                self.show_viewport_controls(ui, available, rect);
            }
            ImageState::Failed(ref msg) => message_indicator(
                ui,
                phosphor::IMAGE_BROKEN,
                &format!("Failed to load image: {msg}"),
            ),
        }

        ui.ctx().pop_events();
    }

    pub fn detect_islands_interaction(&self, pos: Pos2) -> bool {
        self.menu_open || self.islands.iter().any(|rect| rect.contains(pos))
    }

    fn process_events(&mut self, ui: &mut egui::Ui, available: Rect) {
        let layout = LayoutContext::new(available, self.islands.clone());
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

    /// Zoom island at the top right; "Back to image" beside it while the
    /// image is panned out of view.
    fn show_viewport_controls(&mut self, ui: &mut egui::Ui, available: Rect, image_rect: Rect) {
        let t = ui.ctx().get_lb_theme();
        let hit = control_height();
        let font = TypeRole::Body.font_id();
        let painter = ui.painter().clone();
        let text_w = |text: &str| {
            painter
                .layout_no_wrap(text.into(), font.clone(), Color32::PLACEHOLDER)
                .size()
                .x
                + 2.0 * control_space::PAD_X.pts()
        };
        let pct_w = text_w("000%");

        let zoom_rect = Rect::from_min_size(
            egui::pos2(
                available.right() - SCREEN_PADDING.x - island::width(2.0 * hit + pct_w),
                available.top() + SCREEN_PADDING.y,
            ),
            egui::vec2(island::width(2.0 * hit + pct_w), island::height()),
        );
        self.islands.push(zoom_rect);

        let zoom_percentage = (self.master_transform.sx * 100.0).round();
        let mut stop = None;
        island::island(ui, zoom_rect, |ui| {
            let can_zoom_out = zoom_percentage > ZOOM_STEP;
            let out = ui
                .add_enabled_ui(can_zoom_out, |ui| {
                    icon_button_glyph(
                        ui,
                        &t,
                        phosphor::MAGNIFYING_GLASS_MINUS,
                        false,
                        island::fill(&t),
                        hit,
                        TypeRole::Body.size(),
                    )
                })
                .inner;
            tip_text(ui.ctx(), &out, "Zoom out");
            if out.clicked() {
                let target = ((zoom_percentage / ZOOM_STEP).floor() - 1.0) * ZOOM_STEP;
                self.zoom_to(target, available.center());
            }

            let label = if self.master_transform.sx <= MIN_ZOOM_LEVEL {
                "Max".to_string()
            } else {
                format!("{}%", zoom_percentage as i32)
            };
            let pct = Button::secondary(&t, label)
                .min_width(pct_w)
                .max_width(pct_w)
                .show(ui);
            tip_text(ui.ctx(), &pct, "Zoom");
            stop = context_menu::show_click(&pct, &t, |e| {
                e.item_plain("Fit", ZoomStop::Fit);
                for p in [80.0, 100.0, 120.0] {
                    e.item_plain(format!("{p}%"), ZoomStop::Percent(p));
                }
            });

            let into = icon_button_glyph(
                ui,
                &t,
                phosphor::MAGNIFYING_GLASS_PLUS,
                false,
                island::fill(&t),
                hit,
                TypeRole::Body.size(),
            );
            tip_text(ui.ctx(), &into, "Zoom in");
            if into.clicked() {
                let target = ((zoom_percentage / ZOOM_STEP).floor() + 1.0) * ZOOM_STEP;
                self.zoom_to(target, available.center());
            }
        });
        match stop {
            Some(ZoomStop::Fit) => self.reset_viewport(),
            Some(ZoomStop::Percent(p)) => self.zoom_to(p, available.center()),
            None => {}
        }

        if available.intersects(image_rect) {
            return;
        }
        let back_w = island::width(text_w(BACK_LABEL));
        let back_rect = Rect::from_min_size(
            egui::pos2(zoom_rect.left() - Space::Sm.pts() - back_w, zoom_rect.top()),
            egui::vec2(back_w, island::height()),
        );
        self.islands.push(back_rect);
        if island::island(ui, back_rect, |ui| Button::secondary(&t, BACK_LABEL).show(ui)).clicked()
        {
            self.reset_viewport();
        }
    }

    fn show_image_context_menu(&mut self, ui: &mut egui::Ui, available: Rect, image_rect: Rect) {
        // a tap that dismisses an open menu shouldn't also open the native one
        if self.menu_open || !available.intersects(image_rect) {
            return;
        }

        let response = ui.interact(
            available.intersect(image_rect),
            ui.scope_id().with("image_viewer_image"),
            egui::Sense::click(),
        );

        if cfg!(any(target_os = "ios", target_os = "android")) {
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    ui.ctx().set_context_menu(pos, ContextMenuTarget::Image);
                }
            }
            return;
        }

        let t = ui.ctx().get_lb_theme();
        let cmd = context_menu::show(&response, &t, |e| {
            e.item(phosphor::COPY, "Copy image", ImageCmd::Copy);
        });
        if let Some(ImageCmd::Copy) = cmd {
            self.copy_image(ui.ctx());
        }
    }

    pub fn copy_image(&self, ctx: &egui::Context) {
        match self.images.color_image(self.id) {
            Ok(image) => ctx.copy_image(image),
            Err(err) => error!("failed to copy image to clipboard: {err}"),
        }
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
