use egui::Image;

pub struct ImageViewer {
    img: Image<'static>,
}

impl ImageViewer {
    pub fn new(bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        let img = Image::from_bytes("bytes://image.png", bytes);

        Self { img }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.add(self.img.clone()); // nb: doesn't clone the image data
        });
    }
}

pub fn is_supported_image_fmt(ext: &str) -> bool {
    // todo see if this list is incomplete
    const IMG_FORMATS: [&str; 7] = ["png", "jpeg", "jpg", "gif", "webp", "bmp", "ico"];
    IMG_FORMATS.contains(&ext)
}
