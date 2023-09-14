use std::sync::mpsc::Sender;

use eframe::egui;

use lbeditor::{Editor, EditorResponse};

use crate::{
    account::AccountUpdate,
    widgets::{ToolBar, ToolBarVisibility},
};
pub struct Markdown {
    pub editor: Editor,
    pub toolbar: ToolBar,
    update_tx: Sender<AccountUpdate>,
    new_file: bool,
}

impl Markdown {
    pub fn boxed(
        bytes: &[u8], toolbar_visibility: &ToolBarVisibility, update_tx: Sender<AccountUpdate>,
        new_file: bool,
    ) -> Box<Self> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let mut editor = Editor::default();
        editor.set_text(content);

        let toolbar = ToolBar::new(toolbar_visibility);

        Box::new(Self { editor, toolbar, update_tx, new_file })
    }

    pub fn past_first_frame(&self) -> bool {
        self.editor.debug.frame_count > 1
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> EditorResponse {
        ui.vertical(|ui| {
            let res = self.editor.scroll_ui(ui);
            self.toolbar.show(ui, &mut self.editor);
            if self.new_file {
                if let Some(title) = &res.potential_title {
                    self.update_tx
                        .send(AccountUpdate::EditorRenameSignal(title.trim().to_string()))
                        .unwrap();
                }
            }
            res
        })
        .inner
    }
}
