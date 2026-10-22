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
use crate::widgets::diagram_cache::{DiagramCache, DiagramSize};
use crate::widgets::image_cache::{ImageCache, ImageKey, ImageState};

const MIN_ZOOM_LEVEL: f32 = 0.1;
const ZOOM_STEP: f32 = 10.0;
const SCREEN_PADDING: egui::Pos2 =
    if cfg!(target_os = "android") { egui::pos2(10.0, 50.0) } else { egui::pos2(20.0, 20.0) };
pub struct ImageViewer {
    picture: Picture,
    images: ImageCache,
    input_controller: InputController,
    master_transform: Transform,
    /// Last frame's island rects; the canvas ignores presses that start on them.
    islands: Vec<Rect>,
    /// A menu is open over the canvas; touches go to it.
    menu_open: bool,
}

/// What the viewer shows: an image file, or a note's diagram, drawn from
/// its source at the size it's shown so it stays sharp as it's zoomed.
enum Picture {
    File(Uuid),
    Diagram(String),
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
        Self::of(Picture::File(id), images)
    }

    pub fn diagram(source: String, images: ImageCache) -> Self {
        Self::of(Picture::Diagram(source), images)
    }

    fn of(picture: Picture, images: ImageCache) -> Self {
        Self {
            picture,
            images,
            input_controller: InputController::new(InputControllerConfig::new(true, true)),
            master_transform: Transform::identity(),
            islands: Vec::new(),
            menu_open: false,
        }
    }

    /// The image file shown, if it's a file.
    pub fn file(&self) -> Option<Uuid> {
        match self.picture {
            Picture::File(id) => Some(id),
            Picture::Diagram(_) => None,
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

        self.islands.clear();
        match self.state(ui) {
            ImageState::Loading => loading_indicator(ui),
            ImageState::Loaded(texture_id) => {
                let size = self.natural_size(ui, texture_id);
                let rect = self.image_rect(available, size);

                // a diagram's texture is drawn for the size it's shown at
                let texture_id = match &self.picture {
                    Picture::Diagram(source) => {
                        let px = rect.width() * ui.ctx().pixels_per_point();
                        // in steps, so a zoom redraws now and then rather than every frame
                        let px = 2f32.powf((px.max(1.0).log2() * 4.0).ceil() / 4.0) as u32;
                        DiagramCache::of(ui.ctx()).texture(source, px)
                    }
                    Picture::File(_) => Some(texture_id),
                };
                if let Some(texture_id) = texture_id {
                    painter.add(RectShape::filled(rect, 0.0, Color32::WHITE).with_texture(
                        texture_id,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    ));
                }

                self.show_image_context_menu(ui, available, rect);
                self.show_viewport_controls(ui, available, rect);
            }
            ImageState::Failed(ref msg) => {
                let what = match self.picture {
                    Picture::File(_) => "image",
                    Picture::Diagram(_) => "diagram",
                };
                message_indicator(
                    ui,
                    phosphor::IMAGE_BROKEN,
                    &format!("Failed to load {what}: {msg}"),
                )
            }
        }

        ui.ctx().pop_events();
    }

    /// The picture as the image cache would have it. A diagram that has
    /// drawn is `Loaded` with a stand-in texture id; [`Self::show`] asks
    /// for its texture at the size it's shown.
    fn state(&self, ui: &egui::Ui) -> ImageState {
        match &self.picture {
            Picture::File(id) => {
                let state = self.images.get_or_load(&ImageKey::File(*id), true);
                let state = state.lock().unwrap().deref().clone();
                state
            }
            Picture::Diagram(source) => match DiagramCache::of(ui.ctx()).size(source) {
                DiagramSize::Pending => ImageState::Loading,
                DiagramSize::Ready(_) => ImageState::Loaded(egui::TextureId::default()),
                DiagramSize::Failed(message) => ImageState::Failed(message),
            },
        }
    }

    /// The picture's size before the viewer scales it: an image's pixels,
    /// a diagram's points.
    fn natural_size(&self, ui: &egui::Ui, texture_id: egui::TextureId) -> Vec2 {
        match &self.picture {
            Picture::File(_) => {
                let [w, h] = ui.ctx().tex_manager().read().meta(texture_id).unwrap().size;
                Vec2::new(w as f32, h as f32)
            }
            Picture::Diagram(source) => match DiagramCache::of(ui.ctx()).size(source) {
                DiagramSize::Ready(size) => size,
                _ => Vec2::splat(1.0),
            },
        }
    }

    fn back_label(&self) -> &'static str {
        match self.picture {
            Picture::File(_) => "Back to image",
            Picture::Diagram(_) => "Back to diagram",
        }
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
        let zoom_percentage = (self.master_transform.sx * 100.0).round();
        let label = if self.master_transform.sx <= MIN_ZOOM_LEVEL {
            "Max".to_string()
        } else {
            format!("{}%", zoom_percentage as i32)
        };
        // digits needn't be one width: room for the label as it reads
        let pct_w = text_w("000%").max(text_w(&label));

        let zoom_rect = Rect::from_min_size(
            egui::pos2(
                available.right() - SCREEN_PADDING.x - island::width(2.0 * hit + pct_w),
                available.top() + SCREEN_PADDING.y,
            ),
            egui::vec2(island::width(2.0 * hit + pct_w), island::height()),
        );
        self.islands.push(zoom_rect);

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
        let back_label = self.back_label();
        let back_w = island::width(text_w(back_label));
        let back_rect = Rect::from_min_size(
            egui::pos2(zoom_rect.left() - Space::Sm.pts() - back_w, zoom_rect.top()),
            egui::vec2(back_w, island::height()),
        );
        self.islands.push(back_rect);
        if island::island(ui, back_rect, |ui| Button::secondary(&t, back_label).show(ui)).clicked()
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
        let id = match &self.picture {
            Picture::File(id) => *id,
            Picture::Diagram(source) => {
                if let Some((image, _)) = DiagramCache::of(ctx).image(source) {
                    ctx.copy_image(image);
                }
                return;
            }
        };
        match self.images.color_image(id) {
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
