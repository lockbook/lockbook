impl super::App {
    pub fn cut_selected_file(&self) {
        let t = &self.account.tree;
        match t.get_selected_uuid() {
            Some(id) => *t.clipboard.borrow_mut() = Some(id),
            None => self.show_err_dialog("a single file must be selected to copy or cut!"),
        }
    }

    pub fn paste_into_selected_file(&self) {
        let t = &self.account.tree;

        let dest_id = match t.get_selected_uuid() {
            Some(id) => match self.api.file_by_id(id) {
                Ok(meta) => match meta.file_type {
                    lb::FileType::Document => meta.parent,
                    lb::FileType::Folder => meta.id,
                },
                Err(err) => {
                    self.show_err_dialog(&format!("{:?}", err));
                    return;
                }
            },
            None => {
                self.show_err_dialog("a single file must be selected to paste file!");
                return;
            }
        };

        let id = match t.clipboard.borrow_mut().take() {
            Some(id) => id,
            None => {
                self.show_err_dialog("the clipboard is empty, there's nothing to paste!");
                return;
            }
        };

        match self.api.move_file(id, dest_id) {
            Ok(_) => {
                let iter = t.search(id).unwrap();
                t.model.remove(&iter);

                let children = self.api.file_and_all_children(id).unwrap();
                let parent_iter = t.search(dest_id).unwrap();
                t.append_any_children(dest_id, &parent_iter, &children);
            }
            Err(err) => self.show_err_dialog(&format!("{:?}", err)),
        }
    }
}
