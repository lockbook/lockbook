//! Explicit note-to-draft attachment for the native model preview.
use super::*;

impl Chat {
    pub(super) fn show_note_picker(&mut self, ui: &mut Ui) {
        if let Some(result) = self.note_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.note_rx = None;
            match result {
                Ok(text) => {
                    let draft = self.composer.renderer.buffer.current.text.clone();
                    self.composer.set_text(&format!("{draft}{text}"));
                    self.note_picker_open = false;
                }
                Err(error) => {
                    self.note_error = Some(error);
                    self.note_picker_open = true;
                }
            }
        }
        if !self.note_picker_open {
            return;
        }
        let mut open = true;
        let mut selected = None;
        egui::Window::new("Add note to draft")
            .id(Id::new(("apple_note_picker", self.id)))
            .open(&mut open).collapsible(false).default_width(420.0)
            .show(ui.ctx(), |ui| {
                ui.label("Copies the saved note into your draft. You can edit the excerpt before sending.");
                if let Some(error) = &self.note_error { ui.label(error); }
                ui.add(egui::TextEdit::singleline(&mut self.note_filter).hint_text("Find a note…"));
                if self.note_rx.is_some() { ui.spinner(); return; }
                let files = self.composer.renderer.files.read().unwrap();
                let filter = self.note_filter.to_lowercase();
                let mut notes: Vec<_> = files.all_files()
                    .filter(|f| f.is_document() && (f.name.ends_with(".md") || f.name.ends_with(".txt")))
                    .map(|f| (f.id, files.path_segments(f.id).into_iter().map(|(s, _)| s).collect::<Vec<_>>().join("")))
                    // Agent settings and credentials aren't ordinary note attachments.
                    .filter(|(_, path)| !path.split('/').any(|p| p.starts_with('.')))
                    .filter(|(_, path)| path.to_lowercase().contains(&filter))
                    .collect();
                notes.sort_by(|a, b| a.1.cmp(&b.1));
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    if notes.is_empty() { ui.weak("No matching Markdown or text notes."); }
                    for (id, path) in notes {
                        if ui.button(&path).clicked() { selected = Some((id, path)); }
                    }
                });
            });
        self.note_picker_open = open;
        if let Some((id, path)) = selected {
            let core = self.core.clone();
            let ctx = ui.ctx().clone();
            let (tx, rx) = std::sync::mpsc::channel();
            self.note_rx = Some(rx);
            self.note_error = None;
            std::thread::spawn(move || {
                let result = core.read_document(id, false).map_err(|e| format!("Couldn't read note: {e}"))
                    .and_then(|bytes| {
                        if bytes.len() > 12_000 {
                            return Err("This note is too large for the native model preview. Copy a shorter excerpt into the chat instead.".into());
                        }
                        let text = String::from_utf8(bytes).map_err(|_| "This note isn't UTF-8 text.".to_string())?;
                        Ok(format!("\n\nNote excerpt from {path}:\n\n{text}\n\n"))
                    });
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        }
    }
}
