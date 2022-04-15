use std::cell::RefCell;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use crate::ui;
use crate::ui::icons;

#[derive(Clone)]
pub struct FileTree {
    pub cut_files: Rc<RefCell<Option<lb::Uuid>>>,
    pub cols: Vec<FileTreeCol>,
    pub model: gtk::TreeStore,
    pub view: gtk::TreeView,
    pub view_wrapper: gtk::Box,
    pub messages: gtk::Box,
    pub overlay: gtk::Overlay,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum FileTreeCol {
    IconAndName,
    Id,
    Type,
}

impl FileTree {
    pub fn new(account_op_tx: glib::Sender<ui::AccountOp>, hidden_cols: &[String]) -> Self {
        let menu = FileTreeMenu::new(&account_op_tx);

        let cut_files = Rc::new(RefCell::new(None));

        let mut column_types = FileTreeCol::all()
            .iter()
            .map(|_col| glib::Type::STRING)
            .collect::<Vec<glib::Type>>();
        column_types.insert(0, glib::Type::STRING);

        let model = gtk::TreeStore::new(&column_types);
        let view = gtk::TreeView::builder()
            .model(&model)
            .enable_search(false)
            .vexpand(true)
            .build();
        view.connect_columns_changed(|t| t.set_headers_visible(t.columns().len() > 1));
        view.connect_row_activated({
            let tx = account_op_tx.clone();

            move |tview, tpath, _| {
                if tview.row_expanded(tpath) {
                    tview.collapse_row(tpath);
                    return;
                }

                tview.expand_to_path(tpath);
                let model = tview.model().unwrap();
                let iter = model.iter(tpath).unwrap();

                if iter_is_document(&model, &iter) {
                    let id = ui::id_from_tpath(&model, tpath);
                    tx.send(ui::AccountOp::OpenFile(id)).unwrap();
                }
            }
        });

        // Controller for right clicks.
        let rclick = gtk::GestureClick::new();
        rclick.set_button(gdk::ffi::GDK_BUTTON_SECONDARY as u32);
        rclick.set_propagation_phase(gtk::PropagationPhase::Capture);
        rclick.connect_pressed({
            let view = view.clone();
            let menu = menu.clone();
            let cut_files = cut_files.clone();

            move |_, _, x, y| {
                if let Some((Some(tpath), _, _, _)) = view.path_at_pos(x as i32, y as i32) {
                    let sel = view.selection();
                    let (selected_rows, _) = sel.selected_rows();
                    if !selected_rows.contains(&tpath) {
                        sel.unselect_all();
                        sel.select_path(&tpath);
                    }
                }
                menu.update(&view, &cut_files);
                menu.popup_at(x, y);
            }
        });
        view.add_controller(&rclick);

        // Controller for key presses.
        let key_ctlr = gtk::EventControllerKey::new();
        key_ctlr.connect_key_pressed({
            let tx = account_op_tx.clone();

            move |_, key, _, modif| {
                if modif == gdk::ModifierType::CONTROL_MASK {
                    match key {
                        gdk::Key::c => tx.send(ui::AccountOp::CopyFiles).unwrap(),
                        gdk::Key::x => tx.send(ui::AccountOp::CutFiles).unwrap(),
                        gdk::Key::v => tx.send(ui::AccountOp::PasteFiles).unwrap(),
                        _ => {}
                    }
                } else if key == gdk::Key::Delete {
                    tx.send(ui::AccountOp::DeleteFiles).unwrap();
                }
                gtk::Inhibit(false)
            }
        });
        view.add_controller(&key_ctlr);

        // Controller for receiving drops.
        let drop = gtk::DropTarget::new(glib::types::Type::STRING, gdk::DragAction::COPY);
        drop.connect_motion(|_, _x, _y| gdk::DragAction::COPY);
        drop.connect_drop(move |_, val, x, y| {
            account_op_tx
                .send(ui::AccountOp::TreeReceiveDrop(val.clone(), x, y))
                .expect("sending receive-drop account op");
            true
        });
        view.add_controller(&drop);

        let view_wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        view_wrapper.append(&view);
        view_wrapper.append(&menu.popover);

        let messages = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::End)
            .vexpand(false)
            .build();

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&view_wrapper));
        overlay.add_overlay(&messages);

        let sel = view.selection();
        sel.set_mode(gtk::SelectionMode::Multiple);

        let cols = FileTreeCol::all();
        for c in &cols {
            if *c == FileTreeCol::IconAndName || !hidden_cols.contains(&c.name()) {
                view.append_column(&c.as_tree_view_col());
            }
        }

        Self { cut_files, cols, model, view, view_wrapper, messages, overlay }
    }

    pub fn show_msg(&self, msg: &str) {
        let lbl = gtk::Label::new(Some(msg));
        lbl.add_css_class("tree-msg");
        self.messages.append(&lbl);
        glib::timeout_add_seconds_local(3, {
            let messages = self.messages.clone();
            move || {
                messages.remove(&lbl);
                glib::Continue(false)
            }
        });
    }

    pub fn populate(&self, metas: &mut Vec<lb::FileMetadata>) {
        let root = match metas.iter().position(|fm| fm.parent == fm.id) {
            Some(i) => metas.swap_remove(i),
            None => panic!("unable to find root in metadata list"),
        };
        let root_iter = self.append(None, &root);
        self.append_any_children(root.id, &root_iter, metas);
        self.view.expand_row(&self.model.path(&root_iter), false);
    }

    pub fn append_any_children(
        &self, parent_id: lb::Uuid, parent_iter: &gtk::TreeIter, metas: &[lb::FileMetadata],
    ) {
        let children: Vec<&lb::FileMetadata> =
            metas.iter().filter(|fm| fm.parent == parent_id).collect();

        for child in children {
            let item_iter = self.append(Some(parent_iter), child);

            if child.file_type == lb::FileType::Folder {
                self.append_any_children(child.id, &item_iter, metas);
            }
        }
    }

    pub fn append(
        &self, parent_iter: Option<&gtk::TreeIter>, fm: &lb::FileMetadata,
    ) -> gtk::TreeIter {
        let name = &fm.decrypted_name;
        let icon_name = get_icon_name(name, &fm.file_type);
        let id = &fm.id.to_string();
        let ftype = format!("{:?}", fm.file_type);
        self.model.insert_with_values(
            parent_iter,
            None,
            &[(0, &icon_name), (1, name), (2, id), (3, &ftype)],
        )
    }

    pub fn get_selected_uuid(&self) -> Option<lb::Uuid> {
        let (rows, model) = self.view.selection().selected_rows();
        rows.get(0).map(|tpath| ui::id_from_tpath(&model, tpath))
    }

    pub fn get_selected_ids(&self) -> Vec<lb::Uuid> {
        let (rows, model) = self.view.selection().selected_rows();
        rows.iter()
            .map(|tpath| ui::id_from_tpath(&model, tpath))
            .collect()
    }

    pub fn add_file(&self, fm: &lb::FileMetadata) -> Result<(), String> {
        match self.search(fm.parent) {
            Some(parent_iter) => {
                self.append(Some(&parent_iter), fm);
                self.select(fm.id);
                Ok(())
            }
            None => Err(format!("no parent found for file with id '{}'", fm.id)),
        }
    }

    pub fn select(&self, id: lb::Uuid) {
        if let Some(iter) = self.search(id) {
            let p = self.model.path(&iter);
            self.view.expand_to_path(&p);

            let sel = &self.view.selection();
            sel.unselect_all();
            sel.select_iter(&iter);
        }
    }

    pub fn search(&self, id: lb::Uuid) -> Option<gtk::TreeIter> {
        let mut result: Option<gtk::TreeIter> = None;
        self.model.foreach(|model, tpath, iter| -> bool {
            let item_id = ui::id_from_tpath(model, tpath);
            if item_id.eq(&id) {
                result = Some(*iter);
                true
            } else {
                false
            }
        });
        result
    }
}

fn iter_is_document(model: &gtk::TreeModel, iter: &gtk::TreeIter) -> bool {
    model
        .get_value(iter, 3)
        .get::<String>()
        .unwrap_or_else(|_| panic!("getting treeview value: column id {}", 3))
        .eq(&format!("{:?}", lb::FileType::Document))
}

impl FileTreeCol {
    fn all() -> Vec<Self> {
        vec![Self::IconAndName, Self::Id, Self::Type]
    }

    pub fn name(&self) -> String {
        match self {
            FileTreeCol::IconAndName => "Name".to_string(),
            _ => format!("{:?}", self),
        }
    }

    pub fn as_tree_store_index(&self) -> i32 {
        *self as i32 + 1
    }

    pub fn removable() -> Vec<Self> {
        let mut all = Self::all();
        all.retain(|c| !matches!(c, Self::IconAndName));
        all
    }

    pub fn as_tree_view_col(&self) -> gtk::TreeViewColumn {
        let col = gtk::TreeViewColumn::new();
        col.set_title(&self.name());

        let (cell, attr) = (gtk::CellRendererText::new(), "text");
        if *self == FileTreeCol::IconAndName {
            let (renderer_cell, renderer_icon) = (gtk::CellRendererPixbuf::new(), "icon-name");
            renderer_cell.set_padding(4, 0);

            col.pack_start(&renderer_cell, false);
            col.add_attribute(&renderer_cell, renderer_icon, 0);
            col.set_expand(true);
        }

        col.pack_start(&cell, false);
        col.add_attribute(&cell, attr, self.as_tree_store_index());
        col
    }
}

fn get_icon_name(fname: &str, ftype: &lb::FileType) -> String {
    match ftype {
        lb::FileType::Document => ui::document_icon_from_name(fname),
        lb::FileType::Folder => "folder".to_string(),
    }
}

#[derive(Clone)]
struct FileTreeMenu {
    new_document: gtk::Button,
    new_folder: gtk::Button,
    cut: gtk::Button,
    paste: gtk::Button,
    rename: gtk::Button,
    delete: gtk::Button,
    export: gtk::Button,
    popover: gtk::Popover,
}

impl FileTreeMenu {
    fn new(account_op_tx: &glib::Sender<ui::AccountOp>) -> Self {
        let popover = gtk::Popover::builder().halign(gtk::Align::Start).build();

        let new_document = ui::MenuItemBuilder::new()
            .icon(icons::NEW_DOC)
            .label("New Document")
            .popsdown(&popover)
            .build();
        new_document.connect_clicked({
            let tx = account_op_tx.clone();
            move |_| tx.send(ui::AccountOp::NewDocument).unwrap()
        });

        let new_folder = ui::MenuItemBuilder::new()
            .icon(icons::NEW_FOLDER)
            .label("New Folder")
            .popsdown(&popover)
            .build();
        new_folder.connect_clicked({
            let tx = account_op_tx.clone();
            move |_| tx.send(ui::AccountOp::NewFolder).unwrap()
        });

        let cut = ui::MenuItemBuilder::new()
            .icon(icons::CUT)
            .label("Cut")
            .popsdown(&popover)
            .build();
        cut.connect_clicked({
            let tx = account_op_tx.clone();
            move |_| tx.send(ui::AccountOp::CutFiles).unwrap()
        });

        let paste = ui::MenuItemBuilder::new()
            .icon(icons::PASTE)
            .label("Paste")
            .popsdown(&popover)
            .build();
        paste.connect_clicked({
            let tx = account_op_tx.clone();
            move |_| tx.send(ui::AccountOp::PasteFiles).unwrap()
        });

        let rename = ui::MenuItemBuilder::new()
            .icon(icons::RENAME)
            .label("Rename")
            .popsdown(&popover)
            .build();
        rename.connect_clicked({
            let tx = account_op_tx.clone();
            move |_| tx.send(ui::AccountOp::RenameFile).unwrap()
        });

        let delete = ui::MenuItemBuilder::new()
            .icon(icons::DELETE)
            .label("Delete")
            .popsdown(&popover)
            .build();
        delete.connect_clicked({
            let tx = account_op_tx.clone();
            move |_| tx.send(ui::AccountOp::DeleteFiles).unwrap()
        });

        let export = ui::MenuItemBuilder::new()
            .icon(icons::EXPORT)
            .label("Export")
            .popsdown(&popover)
            .build();
        export.connect_clicked({
            let tx = account_op_tx.clone();
            move |_| tx.send(ui::AccountOp::ExportFiles).unwrap()
        });

        let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        menu_box.append(&new_document);
        menu_box.append(&new_folder);
        menu_box.append(&ui::menu_separator());
        menu_box.append(&cut);
        menu_box.append(&paste);
        menu_box.append(&ui::menu_separator());
        menu_box.append(&rename);
        menu_box.append(&delete);
        menu_box.append(&export);
        popover.set_child(Some(&menu_box));

        Self { new_document, new_folder, cut, paste, rename, delete, export, popover }
    }

    fn update(&self, t: &gtk::TreeView, cb: &Rc<RefCell<Option<lb::Uuid>>>) {
        let sel = t.selection();
        let model = t.model().unwrap();

        let is_root_selected = sel.iter_is_selected(&model.iter_first().unwrap());

        let (selected_rows, _) = sel.selected_rows();
        let n_selected = selected_rows.len();

        let at_least_1 = n_selected > 0;
        let only_1 = n_selected == 1;

        self.new_document.set_sensitive(only_1);
        self.new_folder.set_sensitive(only_1);
        self.cut.set_sensitive(only_1 && !is_root_selected);
        self.paste.set_sensitive(only_1 && cb.borrow().is_some());
        self.rename.set_sensitive(only_1 && !is_root_selected);
        self.delete.set_sensitive(at_least_1 && !is_root_selected);
        self.export.set_sensitive(only_1);
    }

    fn popup_at(&self, x: f64, y: f64) {
        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
        self.popover.set_pointing_to(Some(&rect));
        self.popover.popup();
    }
}
