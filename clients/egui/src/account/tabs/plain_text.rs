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
        ui.text_edit_multiline(&mut self.content);
    }
}
