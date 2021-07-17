use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gdk::{DragAction, DragContext, EventButton as GdkEventButton};
use gdk::{EventKey as GdkEventKey, ModifierType};
use gtk::prelude::*;
use gtk::SelectionMode as GtkSelectionMode;
use gtk::TreeIter as GtkTreeIter;
use gtk::TreeModel as GtkTreeModel;
use gtk::TreePath as GtkTreePath;
use gtk::TreeSelection as GtkTreeSelection;
use gtk::TreeStore as GtkTreeStore;
use gtk::TreeView as GtkTreeView;
use gtk::TreeViewColumn as GtkTreeViewColumn;
use gtk::{CellRendererPixbuf, IconSize, Menu as GtkMenu};
use gtk::{
    CellRendererText as GtkCellRendererText, DestDefaults, TargetEntry, TargetFlags, TreeView,
};
use gtk::{Image, Label, MenuItem as GtkMenuItem};
use gtk::{Inhibit as GtkInhibit, SelectionData, TreeIter, TreeStore, TreeViewDropPosition};
use uuid::Uuid;

use lockbook_core::model::client_conversion::ClientFileMetadata;
use lockbook_models::file_metadata::FileType;

use crate::backend::LbCore;
use crate::closure;
use crate::error::LbResult;
use crate::messages::{Messenger, Msg, MsgFn};
use crate::util::gui::RIGHT_CLICK;
use glib::timeout_add_local;
use std::cell::RefCell;

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
    pub fn new(m: &Messenger, c: &Arc<LbCore>, hidden_cols: &Vec<String>) -> Self {
        let popup = Rc::new(FileTreePopup::new(&m));

        let model = GtkTreeStore::new(&Self::tree_store_types());
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

        let hover_last_occurred = Rc::new(RefCell::new(None));

        let targets = [TargetEntry::new(
            "lockbook/files",
            TargetFlags::SAME_WIDGET,
            0,
        )];

        tree.drag_dest_set(DestDefaults::ALL, &targets, DragAction::MOVE);
        tree.drag_source_set(ModifierType::BUTTON1_MASK, &targets, DragAction::MOVE);

        tree.drag_source_set_icon_name("application-x-generic");

        tree.connect_drag_data_received(Self::on_drag_data_received(m, c));
        tree.connect_drag_data_get(Self::on_drag_data_get());
        tree.connect_drag_motion(Self::on_drag_motion(&hover_last_occurred));

        Self { cols, model, tree }
    }

    fn tree_store_types() -> Vec<glib::Type> {
        let mut columns = FileTreeCol::all()
            .iter()
            .map(|_col| glib::Type::String)
            .collect::<Vec<glib::Type>>();

        columns.insert(0, glib::Type::String);

        columns
    }

    fn on_selection_change(popup: &Rc<FileTreePopup>) -> impl Fn(&GtkTreeSelection) {
        closure!(popup => move |tsel| {
            let tree = tsel.get_tree_view().unwrap();
            popup.update(&tree);
        })
    }

    fn on_button_press(
        popup: &Rc<FileTreePopup>,
    ) -> impl Fn(&GtkTreeView, &GdkEventButton) -> GtkInhibit {
        closure!(popup => move |tree, event| {
            if event.get_button() != RIGHT_CLICK {
                return GtkInhibit(false);
            }
            popup.update(&tree);
            popup.menu.popup_at_pointer(Some(event));

            GtkInhibit(Self::inhibit_right_click(tree, event))
        })
    }

    fn on_key_press(m: &Messenger) -> impl Fn(&GtkTreeView, &GdkEventKey) -> GtkInhibit {
        closure!(m => move |_, key| {
            if key.get_hardware_keycode() == DELETE_KEY {
                m.send(Msg::DeleteFiles);
            }
            GtkInhibit(false)
        })
    }

    fn on_row_activated(m: &Messenger) -> impl Fn(&GtkTreeView, &GtkTreePath, &GtkTreeViewColumn) {
        closure!(m => move |t, path, _| {
            if t.row_expanded(&path) {
                t.collapse_row(&path);
                return;
            }

            t.expand_to_path(&path);
            let model = t.get_model().unwrap();
            let iter = model.get_iter(&path).unwrap();

            if Self::iter_is_document(&model, &iter) {
                let iter_id = tree_iter_value!(model, &iter, 2, String);
                let iter_uuid = Uuid::parse_str(&iter_id).unwrap();
                m.send(Msg::OpenFile(Some(iter_uuid)));
            }
        })
    }

    fn on_drag_data_get() -> impl Fn(&TreeView, &DragContext, &SelectionData, u32, u32) {
        |_, _, s, _, _| {
            s.set(&s.get_target(), 8, &[]);
        }
    }

    fn on_drag_data_received(
        m: &Messenger,
        c: &Arc<LbCore>,
    ) -> impl Fn(&TreeView, &DragContext, i32, i32, &SelectionData, u32, u32) {
        closure!(m, c => move |w, d, x, y, _, _, time| {
            if let Some((Some(mut path), pos)) = w.get_dest_row_at_pos(x, y) {
                let model = w.get_model().unwrap().downcast::<TreeStore>().unwrap();

                let mut parent = model.get_iter(&path).unwrap();
                match pos {
                    TreeViewDropPosition::Before |
                    TreeViewDropPosition::After => {
                        path.up();
                        parent = model.get_iter(&path).unwrap();
                    }
                    _ => {
                        if tree_iter_value!(model, &parent, 3, String) == format!("{:?}", FileType::Document)  {
                            path.up();
                            parent = model.get_iter(&path).unwrap();
                        }
                    }
                }

                if tree_iter_value!(model, &parent, 3, String) == format!("{:?}", FileType::Document)  {
                    path.up();
                    parent = model.get_iter(&path).unwrap();
                }

                let (paths, _) = w.get_selection().get_selected_rows();

                let iters = paths.iter().map(|selected| model.get_iter(selected).unwrap()).collect::<Vec<TreeIter>>();
                let ids = iters.iter().map(|iter| Uuid::parse_str(&tree_iter_value!(model, &iter, 2, String)).unwrap()).collect::<Vec<Uuid>>();

                let parent_id = Uuid::parse_str(tree_iter_value!(model, &parent, 2, String).as_str()).unwrap();

                ids.iter().enumerate().for_each(|(index, id)| {
                    match c.move_file(id, parent_id) {
                        Ok(_) => {
                            Self::move_iter(&model, &iters[index], &parent, true);
                            model.remove(&iters[index]);
                        }
                        Err(err) => m.send_err_dialog("moving", err)
                    }
                });

                d.drop_finish(true, time);
            }
        })
    }

    fn on_drag_motion(
        hover_last_occurred: &Rc<RefCell<Option<u32>>>,
    ) -> impl Fn(&TreeView, &DragContext, i32, i32, u32) -> GtkInhibit {
        closure!(hover_last_occurred => move |w, d, x, y, time| {
            if let Some((Some(path), pos)) = w.get_dest_row_at_pos(x, y) {
                let model = w.get_model().unwrap();
                *hover_last_occurred.borrow_mut() = Some(time);

                let pos_corrected =
                    if tree_iter_value!(model, &model.get_iter(&path).unwrap(), 3, String)
                        == format!("{:?}", FileType::Document)
                    {
                        match pos {
                            TreeViewDropPosition::IntoOrBefore => TreeViewDropPosition::Before,
                            TreeViewDropPosition::IntoOrAfter => TreeViewDropPosition::After,
                            _ => pos,
                        }
                    } else {
                        match pos {
                            TreeViewDropPosition::IntoOrBefore
                            | TreeViewDropPosition::IntoOrAfter => {
                                timeout_add_local(
                                    400,
                                    closure!(hover_last_occurred, w, path => move || {
                                    if let Some(t) = *hover_last_occurred.borrow() {
                                        if t == time {
                                            w.expand_row(&path, false);
                                        }
                                    }

                                    Continue(false)
                                }));
                            },
                            _ => {}
                        }

                        pos
                    };

                w.set_drag_dest_row(Some(&path), pos_corrected);
                d.drag_status(d.get_suggested_action(), time);
            }

            GtkInhibit(true)
        })
    }

    fn move_iter(model: &TreeStore, iter: &TreeIter, parent: &TreeIter, is_at_top: bool) {
        let iter_icon = tree_iter_value!(model, &iter, 0, String);
        let iter_name = tree_iter_value!(model, &iter, 1, String);
        let iter_id = tree_iter_value!(model, &iter, 2, String);
        let iter_ftype = tree_iter_value!(model, &iter, 3, String);

        let new_parent = model.insert_with_values(
            Some(&parent),
            None,
            &[0, 1, 2, 3],
            &[&iter_icon, &iter_name, &iter_id, &iter_ftype],
        );

        if let Some(it) = model.iter_children(Some(&iter)) {
            Self::move_iter(model, &it, &new_parent, false);
        }

        if !is_at_top && model.iter_next(&iter) {
            Self::move_iter(model, iter, parent, false);
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

    pub fn refresh(&self, c: &LbCore) -> LbResult<()> {
        let mut expanded_paths = Vec::<GtkTreePath>::new();
        self.search_expanded(&self.iter(), &mut expanded_paths);

        let sel = self.tree.get_selection();
        let (selected_paths, _) = sel.get_selected_rows();

        self.fill(&c)?;

        for path in expanded_paths {
            self.tree.expand_row(&path, false);
        }

        for path in selected_paths {
            sel.select_path(&path);
        }

        Ok(())
    }

    pub fn add(&self, b: &LbCore, f: &ClientFileMetadata) -> LbResult<()> {
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

    pub fn append(
        &self,
        b: &LbCore,
        it: Option<&GtkTreeIter>,
        f: &ClientFileMetadata,
    ) -> LbResult<()> {
        let icon_name = self.get_icon_name(&f.name, &f.file_type);

        let name = &f.name;
        let id = &f.id.to_string();
        let ftype = &format!("{:?}", f.file_type);
        let item_iter =
            self.model
                .insert_with_values(it, None, &[0, 1, 2, 3], &[&icon_name, name, id, ftype]);

        if f.file_type == FileType::Folder {
            let files = b.children(f)?;
            for item in files {
                self.append(b, Some(&item_iter), &item)?;
            }
        }

        Ok(())
    }

    fn get_icon_name(&self, fname: &String, ftype: &FileType) -> String {
        let image_suffixes = vec![
            ".jpg",
            ".jpeg",
            ".png",
            ".pnm",
            ".tga",
            ".farbfeld",
            ".bmp",
            ".draw",
        ];
        let script_suffixes = vec![".sh", ".bash", ".zsh"];

        match ftype {
            FileType::Document => {
                if image_suffixes.iter().any(|suffix| fname.ends_with(suffix)) {
                    "image-x-generic"
                } else if script_suffixes.iter().any(|suffix| fname.ends_with(suffix)) {
                    "text-x-script"
                } else {
                    "text-x-generic"
                }
            }
            FileType::Folder => "folder",
        }
        .to_string()
    }

    pub fn search(&self, iter: &GtkTreeIter, id: &Uuid) -> Option<GtkTreeIter> {
        let iter_id = tree_iter_value!(self.model, iter, 2, String);

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

    pub fn search_expanded(&self, iter: &GtkTreeIter, expanded_paths: &mut Vec<GtkTreePath>) {
        let maybe_path = self.model.get_path(&iter);

        if let Some(path) = maybe_path {
            if self.tree.row_expanded(&path) {
                expanded_paths.push(path);
            }
        }

        if let Some(it) = self.model.iter_children(Some(&iter)) {
            self.search_expanded(&it, expanded_paths)
        }

        if self.model.iter_next(&iter) {
            self.search_expanded(&iter, expanded_paths)
        }
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
        if *col != FileTreeCol::IconAndName {
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
        let mut i = col.to_tree_store_index();
        while i >= 1 {
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
                let iter_id = tree_iter_value!(model, &iter, 2, String);
                Some(Uuid::parse_str(&iter_id).unwrap())
            }
            None => None,
        }
    }

    pub fn iter_is_document(model: &GtkTreeModel, iter: &GtkTreeIter) -> bool {
        tree_iter_value!(model, &iter, 3, String) == format!("{:?}", FileType::Document)
    }

    fn inhibit_right_click(t: &GtkTreeView, e: &GdkEventButton) -> bool {
        let (x, y) = e.get_position();

        if let Some((Some(right_clicked_tpath), _, _, _)) = t.get_path_at_pos(x as i32, y as i32) {
            let (selected_tpaths, _) = t.get_selection().get_selected_rows();
            for tp in selected_tpaths {
                if tp == right_clicked_tpath {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum FileTreeCol {
    IconAndName,
    Id,
    Type,
}

impl FileTreeCol {
    pub fn all() -> Vec<Self> {
        vec![Self::IconAndName, Self::Id, Self::Type]
    }

    pub fn removable() -> Vec<Self> {
        let mut all = Self::all();
        all.retain(|c| !matches!(c, Self::IconAndName));
        all
    }

    pub fn name(&self) -> String {
        match self {
            FileTreeCol::IconAndName => "Name".to_string(),
            _ => format!("{:?}", self),
        }
    }

    fn to_tree_view_col(&self) -> GtkTreeViewColumn {
        let c = GtkTreeViewColumn::new();

        c.set_title(&self.name());

        let (cell, attr) = (GtkCellRendererText::new(), "text");
        if let FileTreeCol::IconAndName = self {
            let (renderer_cell, renderer_icon) = (CellRendererPixbuf::new(), "icon-name");
            renderer_cell.set_padding(4, 0);

            c.pack_start(&renderer_cell, false);
            c.add_attribute(&renderer_cell, renderer_icon, 0);
        }

        c.pack_start(&cell, false);
        c.add_attribute(&cell, attr, self.to_tree_store_index());

        c
    }

    fn to_tree_store_index(&self) -> i32 {
        *self as i32 + 1
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
enum PopupItem {
    NewDocument,
    NewFolder,
    Rename,
    Open,
    Delete,
}

impl PopupItem {
    fn hashmap(m: &Messenger) -> HashMap<Self, GtkMenuItem> {
        let mut items = HashMap::new();
        for (item_key, icon_name, action) in Self::data() {
            let name = if let PopupItem::NewFolder = item_key {
                "New Folder".to_string()
            } else if let PopupItem::NewDocument = item_key {
                "New Document".to_string()
            } else {
                format!("{:?}", item_key)
            };

            let mi = match icon_name {
                None => GtkMenuItem::with_label(&name),
                Some(_) => {
                    let cntr = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    cntr.pack_start(
                        &Image::from_icon_name(icon_name.as_deref(), IconSize::Menu),
                        false,
                        false,
                        0,
                    );
                    cntr.pack_start(&Label::new(Some(&name)), false, false, 10);

                    let mi = GtkMenuItem::new();
                    mi.add(&cntr);
                    mi
                }
            };

            mi.connect_activate(closure!(m => move |_| m.send(action())));
            items.insert(item_key, mi);
        }
        items
    }

    #[rustfmt::skip]
    fn data() -> Vec<(Self, Option<&'static str>, MsgFn)> {
        vec![
            (Self::NewDocument, Some("document-new-symbolic"), || Msg::NewFile(FileType::Document)),
            (Self::NewFolder, Some("folder-new-symbolic"), || Msg::NewFile(FileType::Folder)),
            (Self::Rename, None, || Msg::RenameFile),
            (Self::Open, Some("document-open-symbolic"), || Msg::OpenFile(None)),
            (Self::Delete, Some("edit-delete-symbolic"), || Msg::DeleteFiles),
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
        for (key, _, _) in &PopupItem::data() {
            menu.append(items.get(&key).unwrap());
        }

        Self { items, menu }
    }

    fn update(&self, t: &GtkTreeView) {
        let tsel = t.get_selection();
        let tmodel = t.get_model().unwrap();

        if let Some(iter) = tmodel.get_iter_first() {
            let is_root = tsel.iter_is_selected(&iter);

            let (selected_rows, _) = tsel.get_selected_rows();
            let n_selected = selected_rows.len();

            let at_least_1 = n_selected > 0;
            let only_1 = n_selected == 1;

            for (key, is_enabled) in &[
                (PopupItem::NewFolder, only_1),
                (PopupItem::NewDocument, only_1),
                (PopupItem::Rename, only_1 && !is_root),
                (PopupItem::Open, only_1),
                (PopupItem::Delete, at_least_1),
            ] {
                self.set_enabled(&key, *is_enabled);
            }

            self.menu.show_all();
        }
    }

    fn set_enabled(&self, key: &PopupItem, condition: bool) {
        self.items.get(key).unwrap().set_sensitive(condition);
    }
}

const DELETE_KEY: u16 = 119;
