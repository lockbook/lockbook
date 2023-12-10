use lbeditor::{Editor, EditorResponse};

use crate::widgets::{ToolBar, ToolBarVisibility};
pub struct Markdown {
    pub editor: Editor,
    pub toolbar: ToolBar,
    // update_tx: Sender<AccountUpdate>,
    pub needs_name: bool,
}

impl Markdown {
    // todo: you eleminated the idea of an auto rename signal here, evaluate what to do with it
    pub fn new(
        core: lb::Core, bytes: &[u8], toolbar_visibility: &ToolBarVisibility, needs_name: bool,
    ) -> Self {
        let content = String::from_utf8_lossy(bytes).to_string();
        let mut editor = Editor::new(core);
        editor.set_text(content);

        let toolbar = ToolBar::new(toolbar_visibility);

        Self { editor, toolbar, needs_name }
    }

    pub fn past_first_frame(&self) -> bool {
        self.editor.debug.frame_count > 1
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> EditorResponse {
        ui.vertical(|ui| {
            let mut res = self.editor.scroll_ui(ui);
            self.toolbar.show(ui, &mut self.editor);
            if self.needs_name {
                if let Some(title) = &res.potential_title {
                    res.document_renamed = Some(title.clone());
                }
            }
            res
        })
        .inner
    }
}
