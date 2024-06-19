use egui::Image;

pub struct ImageViewer {
    img: Image<'static>,
}

impl ImageViewer {
    pub fn new(id: &str, ext: &str, bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        let uri = format!("bytes://{}.{}", id, ext);
        let img = Image::from_bytes(uri, bytes);

        Self { img }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.add(self.img.clone()); // nb: doesn't clone the image data
        });
    }
}

pub fn is_supported_image_fmt(ext: &str) -> bool {
    // complete list derived from which features are enabled on image crate according to image-rs default features:
    // https://github.com/image-rs/image/blob/main/Cargo.toml#L70
    const IMG_FORMATS: [&str; 15] = [
        "avif", "bmp", "dds", "exr", "ff", "gif", "hdr", "ico", "jpeg", "png", "pnm", "qoi", "tga",
        "tiff", "webp",
    ];
    IMG_FORMATS.contains(&ext)
}
