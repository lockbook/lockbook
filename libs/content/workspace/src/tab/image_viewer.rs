use std::collections::HashMap;
use std::ops::Deref as _;

use egui::{self, Color32, Pos2, Rect, Vec2};
use epaint::RectShape;
use lb_rs::Uuid;

use crate::widgets::image_cache::{ImageCache, ImageState};

pub struct ImageViewer {
    pub id: Uuid,
    images: ImageCache,
    zoom_factor: f32,
    pan: Vec2,
}

impl ImageViewer {
    pub fn new(id: Uuid, images: ImageCache) -> Self {
        Self { id, images, zoom_factor: 1.0, pan: Vec2::ZERO }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let mut painter = ui.painter().clone();
        painter.set_clip_rect(ui.available_rect_before_wrap());
        painter.rect_filled(painter.clip_rect(), 0., ui.visuals().extreme_bg_color);

        let url = format!("lb://{}", self.id);
        let state = self.images.get_or_load(&url, self.id);
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

                let touch_positions = get_touch_positions(ui);
                let pos_cardinality = touch_positions.len();

                let mut sum_pos = Pos2::default();
                for pos in touch_positions.values() {
                    sum_pos.x += pos.x;
                    sum_pos.y += pos.y;
                }

                let maybe_pos = if pos_cardinality != 0 {
                    Some(sum_pos / pos_cardinality as f32)
                } else {
                    ui.ctx()
                        .pointer_hover_pos()
                        .filter(|&cp| painter.clip_rect().contains(cp))
                };

                let zoom_delta = ui.input(|r| r.zoom_delta());
                self.zoom_factor *= zoom_delta;
                if let Some(pos) = maybe_pos {
                    let relative_pos = ui.available_rect_before_wrap().center() - pos;
                    let pan_correction = egui::vec2(
                        (1.0 - zoom_delta) * relative_pos.x,
                        (1.0 - zoom_delta) * relative_pos.y,
                    );
                    self.pan *= zoom_delta;
                    self.pan -= pan_correction;
                }
                let pan = ui.input(|r| {
                    if let Some(touch_gesture) = r.multi_touch() {
                        touch_gesture.translation_delta
                    } else {
                        r.raw_scroll_delta
                    }
                });
                self.pan += pan;

                let available = ui.available_rect_before_wrap();
                let scale = (available.width() / image_size.x)
                    .min(available.height() / image_size.y)
                    .min(1.0);
                let display_size = image_size * scale * self.zoom_factor;

                let rect = Rect::from_center_size(available.center() + self.pan, display_size);

                painter.add(
                    RectShape::filled(rect, 0.0, Color32::WHITE).with_texture(
                        texture_id,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    ),
                );
            }
            ImageState::Failed(ref msg) => {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("Failed to load image: {msg}"));
                });
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

fn get_touch_positions(ui: &mut egui::Ui) -> HashMap<u64, Pos2> {
    ui.input(|r| {
        let mut touch_positions = HashMap::new();
        for e in r.events.iter() {
            if let egui::Event::Touch { device_id: _, id, phase, pos, force: _ } = *e {
                if phase != egui::TouchPhase::Cancel {
                    touch_positions.insert(id.0, pos);
                }
            }
        }
        touch_positions
    })
}
