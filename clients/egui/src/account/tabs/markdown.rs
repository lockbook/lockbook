use eframe::egui;

use lbeditor::{Editor, EditorResponse};

pub struct Markdown {
    pub editor: Editor,
}

impl Markdown {
    pub fn boxed(bytes: &[u8]) -> Box<Self> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let mut editor = Editor::default();
        editor.set_text(content);

        Box::new(Self { editor })
    }

    pub fn past_first_frame(&self) -> bool {
        self.editor.debug.frame_count > 1
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> EditorResponse {
        ui.vertical(|ui| self.editor.scroll_ui(ui)).inner
    }
}
