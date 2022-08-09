use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn save_file(&self, maybe_id: Option<lb::Uuid>) {
        let maybe_tab = match maybe_id {
            Some(id) => self.account.tab_by_id(id),
            None => self.account.current_tab(),
        };

        if let Some(tab) = maybe_tab {
            if let Some(txt_ed) = tab.content::<ui::TextEditor>() {
                let id = tab.id();
                let buf = txt_ed.editor().buffer();
                let data = buf.text(&buf.start_iter(), &buf.end_iter(), true);
                match self.save_file_content(id, &data) {
                    Ok(()) => self.update_sync_status(),
                    Err(err) => self.show_err_dialog(&format!("error saving: {}", err)),
                }
                self.bg_state.set_last_saved_now(id);
            }
        }
    }

    fn save_file_content(&self, id: lb::Uuid, data: &str) -> Result<(), String> {
        use lb::WriteToDocumentError::*;
        self.core
            .write_document(id, data.as_bytes())
            .map_err(|err| match err {
                lb::Error::UiError(err) => match err {
                    FileDoesNotExist => "file does not exist",
                    FolderTreatedAsDocument => "folder treated as document",
                    InsufficientPermission => todo!(),
                }
                .to_string(),
                lb::Error::Unexpected(msg) => msg,
            })
    }
}
