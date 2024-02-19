use egui_extras::RetainedImage;

pub struct ImageViewer {
    pub bytes: Vec<u8>,
    img: RetainedImage,
}

impl ImageViewer {
    pub fn new(id: impl Into<String>, bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        let img = RetainedImage::from_image_bytes(id, &bytes).unwrap();

        Self { bytes, img }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().show(ui, |ui| {
            if ui.available_width() < self.img.width() as f32
                || ui.available_height() < self.img.height() as f32
            {
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Center).with_cross_justify(true),
                    |ui| {
                        self.img.show(ui);
                    },
                );
            } else {
                self.img.show(ui);
            }
        });
    }
}

pub fn is_supported_image_fmt(ext: &str) -> bool {
    // todo see if this list is incomplete
    const IMG_FORMATS: [&str; 7] = ["png", "jpeg", "jpg", "gif", "webp", "bmp", "ico"];
    IMG_FORMATS.contains(&ext)
}
