use eframe::egui;

pub struct PlainText {
    pub content: String,
}

impl PlainText {
    pub fn boxed(bytes: &[u8]) -> Box<Self> {
        let content = String::from_utf8_lossy(bytes).to_string();

        Box::new(Self { content })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(ui.visuals().extreme_bg_color)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_source("editor")
                    .show(ui, |ui| {
                        egui::TextEdit::multiline(&mut self.content)
                            .desired_width(f32::INFINITY)
                            .frame(false)
                            .margin(egui::vec2(7.0, 7.0))
                            .code_editor()
                            .show(ui);
                    });
            });
    }
}
