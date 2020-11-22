use gdk::EventButton as GdkEventButton;
use gdk::EventKey as GdkEventKey;
use gtk::prelude::*;
use gtk::CellRendererText as GtkCellRendererText;
use gtk::Inhibit as GtkInhibit;
use gtk::Menu as GtkMenu;
use gtk::MenuItem as GtkMenuItem;
use gtk::SelectionMode as GtkSelectionMode;
use gtk::TreeIter as GtkTreeIter;
use gtk::TreeModel as GtkTreeModel;
use gtk::TreePath as GtkTreePath;
use gtk::TreeStore as GtkTreeStore;
use gtk::TreeView as GtkTreeView;
use gtk::TreeViewColumn as GtkTreeViewColumn;
use uuid::Uuid;

use lockbook_core::model::file_metadata::{FileMetadata, FileType};

use crate::backend::LbCore;
use crate::messages::{Messenger, Msg, MsgFn};

#[macro_export]
macro_rules! tree_iter_value {
    ($model:expr, $iter:expr, $id:literal, $type:ty) => {
        $model
            .get_value($iter, $id)
            .get::<$type>()
            .expect(&format!("getting treeview value: column id {}", $id))
            .expect(&format!(
                "getting treeview value: column id {}: mandatory value not found",
                $id
            ));
    };
}

pub struct FileTree {
    cols: Vec<FileTreeCol>,
    model: GtkTreeStore,
    tree: GtkTreeView,
}

impl FileTree {
    pub fn new(m: &Messenger, hidden_cols: &Vec<String>) -> Self {
        let model = GtkTreeStore::new(&FileTreeCol::all_types());

        let tree = GtkTreeView::with_model(&model);
        tree.get_selection().set_mode(GtkSelectionMode::Multiple);
        tree.set_enable_search(false);
        tree.connect_columns_changed(|t| t.set_headers_visible(t.get_columns().len() > 1));
        tree.connect_button_press_event(Self::on_button_press(&m));
        tree.connect_key_press_event(Self::on_key_press(&m));
        tree.connect_row_activated(Self::on_row_activated(&m));

        let cols = FileTreeCol::all();
        for c in &cols {
            if c.name().eq("Name") || !hidden_cols.contains(&c.name()) {
                tree.append_column(&c.to_tree_view_col());
            }
        }

        Self { cols, model, tree }
    }

    fn on_button_press(m: &Messenger) -> impl Fn(&GtkTreeView, &GdkEventButton) -> GtkInhibit {
        let m = m.clone();
        move |tree, event| {
            if event.get_button() != RIGHT_CLICK {
                return GtkInhibit(false);
            }

            let items: Vec<(&str, MsgFn)> = vec![("Delete", || Msg::DeleteFiles)];

            let menu = GtkMenu::new();
            for (name, msg) in items {
                let m = m.clone();

                let mi = GtkMenuItem::with_label(name);
                mi.connect_activate(move |_| m.send(msg()));
                menu.append(&mi);
            }
            menu.show_all();
            menu.popup_at_pointer(Some(event));

            GtkInhibit(Self::inhibit_right_click(tree, event))
        }
    }

    fn on_key_press(m: &Messenger) -> impl Fn(&GtkTreeView, &GdkEventKey) -> GtkInhibit {
        let m = m.clone();
        move |_, key| {
            if key.get_hardware_keycode() == DELETE_KEY {
                m.send(Msg::DeleteFiles);
            }
            GtkInhibit(false)
        }
    }

    fn on_row_activated(m: &Messenger) -> impl Fn(&GtkTreeView, &GtkTreePath, &GtkTreeViewColumn) {
        let m = m.clone();
        move |t, path, _| {
            if t.row_expanded(&path) {
                t.collapse_row(&path);
                m.send(Msg::CloseFile);
                return;
            }

            t.expand_to_path(&path);
            let model = t.get_model().unwrap();
            let iter = model.get_iter(&path).unwrap();
            let iter_id = tree_iter_value!(model, &iter, 1, String);
            let iter_uuid = Uuid::parse_str(&iter_id).unwrap();
            m.send(Msg::OpenFile(iter_uuid));
        }
    }

    pub fn widget(&self) -> &GtkTreeView {
        &self.tree
    }

    pub fn selected_rows(&self) -> (Vec<GtkTreePath>, GtkTreeModel) {
        self.tree.get_selection().get_selected_rows()
    }

    pub fn fill(&self, b: &LbCore) {
        let root = b.root().unwrap();
        self.model.clear();
        self.append_item(b, None, &root);
    }

    pub fn add(&self, b: &LbCore, f: &FileMetadata) {
        let mut file = f.clone();
        let mut parent_iter: Option<GtkTreeIter>;
        while {
            parent_iter = self.search(&self.iter(), &file.parent);
            parent_iter == None
        } {
            file = b.file_by_id(file.parent).unwrap();
        }

        match parent_iter {
            Some(iter) => self.append_item(b, Some(&iter), &file),
            None => panic!("no parent found, should have at least found root!"),
        }

        self.select(&f.id);
    }

    pub fn append_item(&self, b: &LbCore, it: Option<&GtkTreeIter>, f: &FileMetadata) {
        let name = &f.name;
        let id = &f.id.to_string();
        let ftype = &format!("{:?}", f.file_type);
        let item_iter = self
            .model
            .insert_with_values(it, None, &[0, 1, 2], &[name, id, ftype]);

        if f.file_type == FileType::Folder {
            let files = b.children(f).unwrap();
            for item in files {
                self.append_item(b, Some(&item_iter), &item);
            }
        }
    }

    pub fn search(&self, iter: &GtkTreeIter, id: &Uuid) -> Option<GtkTreeIter> {
        let iter_id = tree_iter_value!(self.model, iter, 1, String);

        if iter_id.eq(&id.to_string()) {
            return Some(iter.clone());
        }
        if let Some(it) = self.model.iter_children(Some(&iter)) {
            if let Some(chit) = self.search(&it, id) {
                return Some(chit);
            }
        }
        if self.model.iter_next(&iter) {
            if let Some(it) = self.search(iter, id) {
                return Some(it);
            }
        }
        None
    }

    pub fn select(&self, id: &Uuid) {
        if let Some(it) = self.search(&self.iter(), &id) {
            let p = self.model.get_path(&it).expect("could not get path");
            self.tree.expand_to_path(&p);

            let sel = &self.tree.get_selection();
            sel.unselect_all();
            sel.select_iter(&it);
        }
    }

    pub fn remove(&self, id: &Uuid) {
        if let Some(iter) = self.search(&self.iter(), &id) {
            self.model.remove(&iter);
        }
    }

    pub fn toggle_col(&self, col: &FileTreeCol) {
        if *col != FileTreeCol::Name {
            for c in self.tree.get_columns() {
                if c.get_title().unwrap().eq(&col.name()) {
                    self.tree.remove_column(&c);
                    return;
                }
            }
            self.insert_col(col);
        }
    }

    pub fn insert_col(&self, col: &FileTreeCol) {
        let mut i = *col as i32;
        while i >= 0 {
            i -= 1;
            let prev = self.cols.get(i as usize).unwrap();
            if self.tree_has_col(&prev) {
                self.tree.insert_column(&col.to_tree_view_col(), i + 1);
                return;
            }
        }
    }

    pub fn tree_has_col(&self, col: &FileTreeCol) -> bool {
        for c in self.tree.get_columns() {
            if c.get_title().unwrap().eq(&col.name()) {
                return true;
            }
        }
        false
    }

    fn iter(&self) -> GtkTreeIter {
        self.model.get_iter_first().unwrap()
    }

    pub fn focus(&self) {
        self.tree.grab_focus();
    }

    fn inhibit_right_click(t: &GtkTreeView, e: &GdkEventButton) -> bool {
        let (x, y) = e.get_position();

        if let Some((maybe_tpath, _, _, _)) = t.get_path_at_pos(x as i32, y as i32) {
            if let Some(right_clicked_tpath) = maybe_tpath {
                let (selected_tpaths, _) = t.get_selection().get_selected_rows();
                for tp in selected_tpaths {
                    if tp == right_clicked_tpath {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum FileTreeCol {
    Name,
    Id,
    Type,
}

impl FileTreeCol {
    pub fn all() -> Vec<Self> {
        vec![Self::Name, Self::Id, Self::Type]
    }

    pub fn removable() -> Vec<Self> {
        let mut all = Self::all();
        all.retain(|c| !matches!(c, Self::Name));
        all
    }

    pub fn all_types() -> Vec<glib::Type> {
        Self::all()
            .iter()
            .map(|_| glib::Type::String)
            .collect::<Vec<glib::Type>>()
    }

    pub fn name(&self) -> String {
        format!("{:?}", self)
    }

    fn to_tree_view_col(&self) -> GtkTreeViewColumn {
        let cell = GtkCellRendererText::new();
        cell.set_padding(8, 0);

        let c = GtkTreeViewColumn::new();
        c.set_title(&self.name());
        c.pack_start(&cell, true);
        c.add_attribute(&cell, "text", *self as i32);
        c
    }
}

const DELETE_KEY: u16 = 119;
const RIGHT_CLICK: u32 = 3;
