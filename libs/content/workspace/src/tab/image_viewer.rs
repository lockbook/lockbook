use egui::Image;
use lb_rs::Uuid;

pub struct ImageViewer {
    pub id: Uuid,
    zoom_factor: f32,
    pan: egui::Vec2,
    img: Image<'static>,
}

impl ImageViewer {
    pub fn new(id: Uuid, ext: &str, bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        let uri = format!("bytes://{}.{}", id, ext);
        let img = Image::from_bytes(uri, bytes);

        Self { id, img, zoom_factor: 1.0, pan: egui::Vec2::ZERO }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let tlr = self.img.load_for_size(ui.ctx(), ui.available_size());
        let original_image_size = tlr.as_ref().ok().and_then(|t| t.size());
        let ui_size = self.img.calc_size(ui.available_size(), original_image_size);

        // handle input and save pan/zoom levels
        let zoom_delta = ui.input(|r| r.zoom_delta());
        self.zoom_factor *= zoom_delta;
        if let Some(hover_pos) = ui.input(|r| r.pointer.hover_pos()) {
            let relative_pos = ui.available_rect_before_wrap().center() - hover_pos;

            let pan_correction = egui::vec2(
                (1.0 - zoom_delta) * relative_pos.x,
                (1.0 - zoom_delta) * relative_pos.y,
            );

            self.pan = self.pan * zoom_delta;
            self.pan = self.pan - pan_correction;
        }
        self.pan = self.pan + ui.input(|r| r.smooth_scroll_delta);

        // draw the image according to pan and zoom levels
        let texture_id = tlr.as_ref().ok().and_then(|t| t.texture_id()).unwrap();
        let rect = egui::Rect::from_center_size(
            ui.available_rect_before_wrap().center() + self.pan,
            ui_size * self.zoom_factor,
        );

        let mut painter = ui.painter().clone();
        // avoid overlapping the tab strip
        painter.set_clip_rect(ui.available_rect_before_wrap());

        painter.rect_filled(painter.clip_rect(), 0., ui.visuals().extreme_bg_color);

        painter.image(
            texture_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
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
