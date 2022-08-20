use eframe::egui;
use egui_extras::RetainedImage;

pub struct ImageViewer {
    img: RetainedImage,
}

impl ImageViewer {
    pub fn boxed(id: impl Into<String>, bytes: &[u8]) -> Box<Self> {
        let img = RetainedImage::from_image_bytes(id, bytes).unwrap();

        Box::new(Self { img })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.img.show(ui);
    }
}
