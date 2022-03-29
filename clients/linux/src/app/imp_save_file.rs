use gtk::prelude::*;

impl super::App {
    pub fn save_file(&self, maybe_id: Option<lb::Uuid>) {
        let maybe_tab = match maybe_id {
            Some(id) => self.account.tab_by_id(id),
            None => self.account.current_tab(),
        };

        if let Some(tab) = maybe_tab {
            let id = tab.id();
            let b = tab.editor().buffer();
            let data = b.text(&b.start_iter(), &b.end_iter(), true);
            match self.save_file_content(&id, &data) {
                Ok(()) => self.update_sync_status(),
                Err(err) => eprintln!("error saving: {}", err),
            }
            self.bg_state.set_last_saved_now(id);
        }
    }

    fn save_file_content(&self, id: &lb::Uuid, data: &str) -> Result<(), String> {
        use lb::WriteDocumentError::*;
        self.api
            .write_document(*id, data.as_bytes())
            .map_err(|err| match err {
                lb::Error::UiError(err) => match err {
                    NoAccount => "no account",
                    FileDoesNotExist => "file does not exist",
                    FolderTreatedAsDocument => "folder treated as document",
                }
                .to_string(),
                lb::Error::Unexpected(msg) => msg,
            })
    }
}
