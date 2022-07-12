use gtk::glib;
use gtk::prelude::*;

use crate::ui;

#[derive(Clone)]
struct FileInfo {
    id: lb::Uuid,
    ftype: lb::FileType,
    path: String,
    all_children: Vec<lb::FileMetadata>,
}

impl super::App {
    pub fn delete_files(&self) {
        let files = self.get_selected_file_infos().unwrap();
        if files.is_empty() {
            return;
        }

        let tree = build_tree(&files);

        let are_you_sure_lbl = gtk::Label::builder()
            .label("Are you sure you want to delete the following files?")
            .margin_top(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        let d = gtk::Dialog::builder()
            .transient_for(&self.window)
            .title("Confirm Delete")
            .modal(true)
            .build();

        d.content_area().append(&are_you_sure_lbl);
        d.content_area().append(&tree);
        d.set_default_response(gtk::ResponseType::Cancel);
        d.add_button("No", gtk::ResponseType::Cancel);
        d.add_button("I'm Sure", gtk::ResponseType::Yes);

        let app = self.clone();
        d.connect_response(move |d, resp| {
            if resp != gtk::ResponseType::Yes {
                d.close();
                return;
            }

            let files_to_delete = files
                .clone()
                .into_iter()
                .filter(|f| !covered_by_another_selection(&files, f))
                .collect::<Vec<FileInfo>>();

            for info in &files_to_delete {
                if let Err(err) = app.api.delete_file(info.id) {
                    app.show_err_dialog(&format!("{:?}", err));
                    break;
                }

                // Remove the file from the file tree.
                if let Some(iter) = app.account.tree.search(info.id) {
                    app.account.tree.model.remove(&iter);
                }

                // Close the tab of any file or its children if opened.
                for child in &info.all_children {
                    if let Some(tab) = app.account.tab_by_id(child.id) {
                        let t = &app.account.tabs;
                        t.remove_page(t.page_num(&tab));
                    }
                }
            }

            d.close();
            app.update_sync_status();
        });

        d.show();
    }

    fn get_selected_file_infos(&self) -> Result<Vec<FileInfo>, String> {
        let mut files = Vec::new();

        let (selected_rows, model) = self.account.tree.view.selection().selected_rows();
        for tpath in &selected_rows {
            let id = ui::id_from_tpath(&model, tpath);

            let path = self
                .api
                .get_path_by_id(id)
                .map_err(|err| format!("{:?}", err))?;

            let meta = self
                .api
                .get_file_by_id(id)
                .map_err(|err| format!("{:?}", err))?;

            let ftype = meta.file_type;

            let all_children = match meta.file_type {
                lb::FileType::Document => vec![],
                lb::FileType::Folder => self
                    .api
                    .get_and_get_children_recursively(id)
                    .map_err(|err| format!("{:?}", err))?,
            };

            files.push(FileInfo { id, ftype, path, all_children });
        }

        Ok(files)
    }
}

fn build_tree(files: &[FileInfo]) -> gtk::TreeView {
    let model = gtk::TreeStore::new(&[glib::Type::STRING, glib::Type::STRING]);
    let tree = gtk::TreeView::builder()
        .model(&model)
        .enable_search(false)
        .can_focus(false)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    tree.selection().set_mode(gtk::SelectionMode::None);
    tree_add_col(&tree, "Name", 0);
    let mut has_folder = false;
    for f in files {
        let n_children = match f.ftype {
            lb::FileType::Document => "-".to_string(),
            lb::FileType::Folder => {
                has_folder = true;
                format!("{}", f.all_children.len())
            }
        };
        model.insert_with_values(None, None, &[(0, &f.path), (1, &n_children)]);
    }
    if has_folder {
        tree_add_col(&tree, "Children", 1);
    }
    tree
}

fn tree_add_col(tree: &gtk::TreeView, name: &str, id: i32) {
    let cell = gtk::CellRendererText::new();
    cell.set_padding(12, 4);

    let c = gtk::TreeViewColumn::new();
    c.set_title(name);
    c.pack_start(&cell, true);
    c.add_attribute(&cell, "text", id);
    tree.append_column(&c);
}

fn covered_by_another_selection(files: &[FileInfo], info: &FileInfo) -> bool {
    for selected_file in files {
        for ch in &selected_file.all_children {
            if !selected_file.id.eq(&ch.id) && info.id.eq(&ch.id) {
                return true;
            }
        }
    }
    false
}
