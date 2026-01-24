use egui::Image;
use lb_rs::{LbErrKind, LbResult, Uuid};

use super::svg_editor::SVGEditor;

pub struct ImageViewer {
    pub id: Uuid,
    zoom_factor: f32,
    pan: egui::Vec2,
    img: Image<'static>,
}

impl ImageViewer {
    pub fn new(id: Uuid, ext: &str, bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        let uri = format!("bytes://{id}.{ext}");
        let img = Image::from_bytes(uri, bytes);

        Self { id, img, zoom_factor: 1.0, pan: egui::Vec2::ZERO }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> LbResult<()> {
        let mut painter = ui.painter().clone();
        // avoid overlapping the tab strip
        painter.set_clip_rect(ui.available_rect_before_wrap());

        painter.rect_filled(painter.clip_rect(), 0., ui.visuals().extreme_bg_color);

        let tlr = self.img.load_for_size(ui.ctx(), ui.available_size());
        let original_image_size = tlr.as_ref().ok().and_then(|t| t.size());
        let ui_size = self.img.calc_size(ui.available_size(), original_image_size);

        let touch_positions = SVGEditor::get_touch_positions(ui);
        let pos_cardinality = touch_positions.len();

        let mut sum_pos = egui::Pos2::default();
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

        // handle input and save pan/zoom levels
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

        // draw the image according to pan and zoom levels
        let texture_id = tlr
            .as_ref()
            .ok()
            .and_then(|t| t.texture_id())
            .ok_or(LbErrKind::Unexpected("failed to load the image's texture".to_owned()))?;
        let rect = egui::Rect::from_center_size(
            ui.available_rect_before_wrap().center() + self.pan,
            ui_size * self.zoom_factor,
        );

        painter.image(
            texture_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        Ok(())
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
