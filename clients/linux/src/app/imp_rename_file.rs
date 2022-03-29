use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn rename_file(&self) {
        let meta = match self.get_selected_metadata() {
            Ok(v) => v,
            Err(err) => {
                self.show_err_dialog(&err);
                return;
            }
        };

        let entry = gtk::Entry::new();
        entry.buffer().set_text(&meta.decrypted_name);
        entry.set_width_request(250);
        entry.select_region(0, -1);

        let err_lbl = gtk::Label::new(None);
        err_lbl.set_widget_name("err");

        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.append(&entry);

        let tview = &self.account.tree.view;
        let n_cols = tview.n_columns() as i32;
        let (selected_rows, _) = tview.selection().selected_rows();

        let mut rect = tview.cell_area(selected_rows.get(0), tview.column(n_cols - 1).as_ref());
        if n_cols > 1 {
            rect.set_y(rect.y() + 24);
        }

        let popover = gtk::Popover::builder()
            .pointing_to(&rect)
            .position(gtk::PositionType::Right)
            .valign(gtk::Align::Center)
            .child(&content)
            .autohide(true)
            .build();

        let tree_cntr = self.account.tree.cntr.clone();
        tree_cntr.append(&popover);
        popover.connect_closed(move |p| tree_cntr.remove(p));

        let app = self.clone();
        let p = popover.clone();
        entry.connect_activate(move |entry| {
            let id = meta.id;
            let new_name = entry.buffer().text();
            if let Err(err) = app.api.rename_file(id, &new_name) {
                err_lbl.set_text(&format!("{:?}", err)); // todo
                content.append(&err_lbl);
                if matches!(err, lb::Error::Unexpected(_)) {
                    content.remove(entry);
                }
                return;
            }

            p.popdown();

            // Update the name (and icon if necessary) in the filetree.
            let tree = &app.account.tree;
            let iter = tree
                .search(&id)
                .unwrap_or_else(|| panic!("renaming file tree entry: none found for id '{}'", &id));
            if meta.file_type == lb::FileType::Document {
                tree.model
                    .set_value(&iter, 0, &ui::document_icon_from_name(&new_name).to_value());
            }
            tree.model.set_value(&iter, 1, &new_name.to_value());

            // Update the tab label if the file is open.
            if let Some(tab) = app.account.tab_by_id(id) {
                app.account.tabs.set_tab_label_text(&tab, &new_name);
                // todo: if it's an unsupported file extension, close the editor
            }

            app.update_sync_status();
        });

        popover.popup();
    }

    fn get_selected_metadata(&self) -> Result<lb::FileMetadata, String> {
        let id = self
            .account
            .tree
            .get_selected_uuid()
            .ok_or("no file selected!")?;
        let meta = self
            .api
            .file_by_id(id)
            .map_err(|err| format!("getting current file name: {:?}", err))?;
        Ok(meta)
    }
}
