use uuid::Uuid;

use glib::clone;
use gtk::prelude::*;
use gtk::{
    CellRendererText as GtkCellRendererText, TreeIter as GtkTreeIter, TreeStore as GtkTreeStore,
    TreeView as GtkTreeView, TreeViewColumn as GtkTreeViewColumn,
};

use lockbook_core::model::file_metadata::{FileMetadata, FileType};

use crate::backend::LbCore;
use crate::messages::{Messenger, Msg};

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
    pub tree: GtkTreeView,
}

impl FileTree {
    pub fn new(m: &Messenger, hidden_cols: &Vec<String>) -> Self {
        let model = GtkTreeStore::new(&FileTreeCol::all_types());

        let tree = GtkTreeView::with_model(&model);
        tree.connect_columns_changed(|t| {
            t.set_headers_visible(t.get_columns().len() > 1);
        });
        tree.connect_row_activated(clone!(@strong m => move |t, path, _| {
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
        }));

        let cols = FileTreeCol::all();
        for c in &cols {
            if c.name().eq("Name") || !hidden_cols.contains(&c.name()) {
                tree.append_column(&c.to_tree_view_col());
            }
        }

        Self { tree, model, cols }
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
            self.tree.get_selection().select_iter(&it);
        }
    }

    /*pub fn remove(&self, id: &Uuid) {
        if let Some(iter) = self.search(&self.iter(), &id) {
            self.model.remove(&iter);
        }
    }*/

    pub fn toggle_col(&self, col: &FileTreeCol) {
        match col {
            FileTreeCol::Name => {}
            _ => {
                for c in self.tree.get_columns() {
                    if c.get_title().unwrap().eq(&col.name()) {
                        self.tree.remove_column(&c);
                        return;
                    }
                }
                self.insert_col(col);
            }
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
}

#[derive(Debug, Clone, Copy)]
pub enum FileTreeCol {
    Name,
    ID,
    Type,
}

impl FileTreeCol {
    pub fn all() -> Vec<Self> {
        vec![Self::Name, Self::ID, Self::Type]
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
        let c = GtkTreeViewColumn::new();
        let cell = GtkCellRendererText::new();

        c.set_title(&self.name());
        c.pack_start(&cell, true);
        c.add_attribute(&cell, "text", *self as i32);
        c
    }
}
