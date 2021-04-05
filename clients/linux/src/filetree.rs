use std::collections::HashMap;
use std::rc::Rc;

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
use gtk::TreeSelection as GtkTreeSelection;
use gtk::TreeStore as GtkTreeStore;
use gtk::TreeView as GtkTreeView;
use gtk::TreeViewColumn as GtkTreeViewColumn;
use uuid::Uuid;

use lockbook_models::file_metadata::{FileMetadata, FileType};

use crate::backend::LbCore;
use crate::error::LbResult;
use crate::messages::{Messenger, Msg, MsgFn};
use crate::util::gui::RIGHT_CLICK;

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
        let popup = Rc::new(FileTreePopup::new(&m));

        let model = GtkTreeStore::new(&FileTreeCol::all_types());
        let tree = GtkTreeView::with_model(&model);
        tree.set_enable_search(false);
        tree.connect_columns_changed(|t| t.set_headers_visible(t.get_columns().len() > 1));
        tree.connect_button_press_event(Self::on_button_press(&popup));
        tree.connect_key_press_event(Self::on_key_press(&m));
        tree.connect_row_activated(Self::on_row_activated(&m));

        let sel = tree.get_selection();
        sel.connect_changed(Self::on_selection_change(&popup));
        sel.set_mode(GtkSelectionMode::Multiple);

        let cols = FileTreeCol::all();
        for c in &cols {
            if c.name().eq("Name") || !hidden_cols.contains(&c.name()) {
                tree.append_column(&c.to_tree_view_col());
            }
        }

        Self { cols, model, tree }
    }

    fn on_selection_change(popup: &Rc<FileTreePopup>) -> impl Fn(&GtkTreeSelection) {
        let popup = popup.clone();
        move |tsel| {
            let tree = tsel.get_tree_view().unwrap();
            popup.update(&tree);
        }
    }

    fn on_button_press(
        popup: &Rc<FileTreePopup>,
    ) -> impl Fn(&GtkTreeView, &GdkEventButton) -> GtkInhibit {
        let popup = popup.clone();

        move |tree, event| {
            if event.get_button() != RIGHT_CLICK {
                return GtkInhibit(false);
            }

            popup.update(&tree);
            popup.menu.popup_at_pointer(Some(event));

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
                return;
            }

            t.expand_to_path(&path);
            let model = t.get_model().unwrap();
            let iter = model.get_iter(&path).unwrap();

            if Self::iter_is_document(&model, &iter) {
                let iter_id = tree_iter_value!(model, &iter, 1, String);
                let iter_uuid = Uuid::parse_str(&iter_id).unwrap();
                m.send(Msg::OpenFile(Some(iter_uuid)));
            }
        }
    }

    pub fn widget(&self) -> &GtkTreeView {
        &self.tree
    }

    pub fn selected_rows(&self) -> (Vec<GtkTreePath>, GtkTreeModel) {
        self.tree.get_selection().get_selected_rows()
    }

    pub fn fill(&self, c: &LbCore) -> LbResult<()> {
        self.model.clear();
        let root = c.root()?;
        self.append(c, None, &root)
    }

    pub fn add(&self, b: &LbCore, f: &FileMetadata) -> LbResult<()> {
        let mut file = f.clone();
        let mut parent_iter: Option<GtkTreeIter>;
        while {
            parent_iter = self.search(&self.iter(), &file.parent);
            parent_iter == None
        } {
            file = b.file_by_id(file.parent)?;
        }

        match parent_iter {
            Some(iter) => self.append(b, Some(&iter), &file)?,
            None => panic!("no parent found, should have at least found root!"),
        }

        self.select(&f.id);
        Ok(())
    }

    pub fn append(&self, b: &LbCore, it: Option<&GtkTreeIter>, f: &FileMetadata) -> LbResult<()> {
        let name = &f.name;
        let id = &f.id.to_string();
        let ftype = &format!("{:?}", f.file_type);
        let item_iter = self
            .model
            .insert_with_values(it, None, &[0, 1, 2], &[name, id, ftype]);

        if f.file_type == FileType::Folder {
            let files = b.children(f)?;
            for item in files {
                self.append(b, Some(&item_iter), &item)?;
            }
        }

        Ok(())
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

    pub fn set_name(&self, id: &Uuid, name: &str) {
        if let Some(iter) = self.search(&self.iter(), &id) {
            self.model.set(&iter, &[0], &[&name.to_string()]);
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

    pub fn get_selected_uuid(&self) -> Option<Uuid> {
        let (rows, model) = self.tree.get_selection().get_selected_rows();
        match rows.get(0) {
            Some(tpath) => {
                let iter = model.get_iter(&tpath).unwrap();
                let iter_id = tree_iter_value!(model, &iter, 1, String);
                Some(Uuid::parse_str(&iter_id).unwrap())
            }
            None => None,
        }
    }

    pub fn iter_is_document(model: &GtkTreeModel, iter: &GtkTreeIter) -> bool {
        tree_iter_value!(model, &iter, 2, String) == "Document"
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

#[derive(Hash, Eq, PartialEq, Debug)]
enum PopupItem {
    Rename,
    Open,
    Delete,
}

impl PopupItem {
    fn hashmap(m: &Messenger) -> HashMap<Self, GtkMenuItem> {
        let mut items = HashMap::new();
        for (item_key, action) in Self::data() {
            let m = m.clone();

            let mi = GtkMenuItem::with_label(&format!("{:?}", item_key));
            mi.connect_activate(move |_| m.send(action()));
            items.insert(item_key, mi);
        }
        items
    }

    #[rustfmt::skip]
    fn data() -> Vec<(Self, MsgFn)> {
        vec![
            (Self::Rename, || Msg::RenameFile),
            (Self::Open, || Msg::OpenFile(None)),
            (Self::Delete, || Msg::DeleteFiles),
        ]
    }
}

struct FileTreePopup {
    items: HashMap<PopupItem, GtkMenuItem>,
    menu: GtkMenu,
}

impl FileTreePopup {
    fn new(m: &Messenger) -> Self {
        let items = PopupItem::hashmap(&m);
        let menu = GtkMenu::new();
        for (key, _) in &PopupItem::data() {
            menu.append(items.get(&key).unwrap());
        }

        Self { items, menu }
    }

    fn update(&self, t: &GtkTreeView) {
        let tsel = t.get_selection();
        let tmodel = t.get_model().unwrap();

        let (selected_rows, _) = tsel.get_selected_rows();
        let n_selected = selected_rows.len();

        let at_least_1 = n_selected > 0;
        let only_1 = n_selected == 1;
        let is_root = tsel.iter_is_selected(&tmodel.get_iter_first().unwrap());

        for (key, is_enabled) in &[
            (PopupItem::Rename, only_1 && !is_root),
            (PopupItem::Open, only_1),
            (PopupItem::Delete, at_least_1),
        ] {
            self.set_enabled(&key, *is_enabled);
        }

        self.menu.show_all();
    }

    fn set_enabled(&self, key: &PopupItem, condition: bool) {
        self.items.get(key).unwrap().set_sensitive(condition);
    }
}

const DELETE_KEY: u16 = 119;
