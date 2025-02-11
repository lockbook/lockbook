use egui::Image;
use lb_rs::Uuid;

pub struct ImageViewer {
    pub id: Uuid,

    img: Image<'static>,
}

impl ImageViewer {
    pub fn new(id: Uuid, ext: &str, bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        let uri = format!("bytes://{}.{}", id, ext);
        let img = Image::from_bytes(uri, bytes);

        Self { id, img }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.add(self.img.clone()); // nb: doesn't clone the image data
        });
    }
}

// a copy of this fn exists in Swift as isSupportedImageFormat()
pub fn is_supported_image_fmt(ext: &str) -> bool {
    // complete list derived from which features are enabled on image crate according to image-rs default features:
    // https://github.com/image-rs/image/blob/main/Cargo.toml#L70
    const IMG_FORMATS: [&str; 16] = [
        "avif", "bmp", "dds", "exr", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "png", "pnm", "qoi",
        "tga", "tiff", "webp",
    ];
    IMG_FORMATS.contains(&ext)
}
