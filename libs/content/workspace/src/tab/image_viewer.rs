use std::ops::Deref as _;

use egui::{self, Color32, Pos2, Rect, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;
use resvg::usvg::Transform;

use crate::tab::input_controller::{
    InputController, InputControllerConfig, InputControllerEvent, LayoutContext,
};
use crate::widgets::image_cache::{ImageCache, ImageState};

pub struct ImageViewer {
    pub id: Uuid,
    images: ImageCache,
    input_controller: InputController,
    viewport_transform: Transform,
}

impl ImageViewer {
    pub fn new(id: Uuid, images: ImageCache) -> Self {
        Self {
            id,
            images,
            input_controller: InputController::new(InputControllerConfig::new(false, true)),
            viewport_transform: Transform::identity(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
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

                let scale = (available.width() / image_size.x)
                    .min(available.height() / image_size.y)
                    .min(1.0);
                let display_size =
                    image_size * scale * self.viewport_transform.sx.max(f32::EPSILON);

                let rect = Rect::from_center_size(
                    available.center()
                        + Vec2::new(self.viewport_transform.tx, self.viewport_transform.ty),
                    display_size,
                );

                painter.add(RectShape::filled(rect, 0.0, Color32::WHITE).with_texture(
                    texture_id,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                ));
            }
            ImageState::Failed(ref msg) => {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("Failed to load image: {msg}"));
                });
            }
        }
    }

    fn process_events(&mut self, ui: &mut egui::Ui, available: Rect) {
        let layout = LayoutContext::new(available, Vec::new());
        for event in self.input_controller.process(ui, &layout) {
            if let InputControllerEvent::ViewportChange(transform) = event {
                self.viewport_transform = self.viewport_transform.post_concat(transform);
            }
        }
    }
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
