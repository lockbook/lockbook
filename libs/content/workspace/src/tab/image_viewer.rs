use egui::{pos2, Image, Rect};

pub struct ImageViewer {
    img: Image<'static>,
}

impl ImageViewer {
    pub fn new(bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        let img = Image::from_bytes("bytes://qr.png", bytes);

        Self { img }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().show(ui, |ui| {
            let img_size = self.img.calc_size(ui.available_size(), None);
            let ui_size = ui.available_size();
            if img_size.x < ui_size.x || img_size.y < ui_size.y {
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Center).with_cross_justify(true),
                    |ui| {
                        self.img
                            .paint_at(ui, Rect::from_min_size(pos2(0.0, 0.0), img_size));
                    },
                );
            } else {
                self.img
                    .paint_at(ui, Rect::from_min_size(pos2(0.0, 0.0), img_size));
            }
        });
    }
}

pub fn is_supported_image_fmt(ext: &str) -> bool {
    // todo see if this list is incomplete
    const IMG_FORMATS: [&str; 7] = ["png", "jpeg", "jpg", "gif", "webp", "bmp", "ico"];
    IMG_FORMATS.contains(&ext)
}
