use eframe::egui;

use lbeditor::Editor;

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

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_source("editor")
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    self.editor.ui(ui);
                });
            });
    }
}
