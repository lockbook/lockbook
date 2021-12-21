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
use gtk::{CellRendererPixbuf, Menu as GtkMenu};
use gtk::{
    CellRendererText as GtkCellRendererText, DestDefaults, TargetEntry, TargetFlags, TreeView,
};
use gtk::{Inhibit as GtkInhibit, SelectionData, TreeIter, TreeStore, TreeViewDropPosition};
use gtk::{MenuItem as GtkMenuItem, TargetList};
use uuid::Uuid;

use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};

use crate::backend::LbCore;
use crate::error::{LbError, LbResult};
use crate::messages::{Messenger, Msg, MsgFn};
use crate::util::gui::RIGHT_CLICK;
use crate::util::{LOCKBOOK_FILES_TARGET_INFO, URI_TARGET_INFO};
use crate::{closure, progerr};
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
            ))
    };
}

pub struct FileTree {
    cols: Vec<FileTreeCol>,
    model: GtkTreeStore,
    tree: GtkTreeView,
}

impl FileTree {
    pub fn new(m: &Messenger, c: &Arc<LbCore>, hidden_cols: &Vec<String>) -> Self {
        let popup = Rc::new(FileTreePopup::new(m));

        let mut column_types = FileTreeCol::all()
            .iter()
            .map(|_col| glib::Type::String)
            .collect::<Vec<glib::Type>>();

        column_types.insert(0, glib::Type::String);

        let model = GtkTreeStore::new(&column_types);
        let tree = GtkTreeView::with_model(&model);
        tree.set_enable_search(false);
        tree.connect_columns_changed(|t| t.set_headers_visible(t.get_columns().len() > 1));
        tree.connect_button_press_event(Self::on_button_press(&popup));
        tree.connect_key_press_event(Self::on_key_press(m));
        tree.connect_row_activated(Self::on_row_activated(m));

        let sel = tree.get_selection();
        sel.connect_changed(Self::on_selection_change(&popup));
        sel.set_mode(GtkSelectionMode::Multiple);

        let cols = FileTreeCol::all();

        for c in &cols {
            if c.name().eq("Name") || !hidden_cols.contains(&c.name()) {
                tree.append_column(&c.as_tree_view_col());
            }
        }

        let drag_hover_last_occurred = Rc::new(RefCell::new(None));
        let drag_ends_last_occurred = Rc::new(RefCell::new(None));

        let targets = [TargetEntry::new(
            "lockbook/files",
            TargetFlags::SAME_WIDGET,
            LOCKBOOK_FILES_TARGET_INFO,
        )];

        tree.drag_dest_set(DestDefaults::ALL, &targets, DragAction::COPY);
        tree.drag_source_set(ModifierType::BUTTON1_MASK, &targets, DragAction::MOVE);

        let target_list = TargetList::new(&targets);
        target_list.add_uri_targets(URI_TARGET_INFO);

        tree.drag_dest_set_target_list(Some(&target_list));

        tree.drag_source_set_icon_name("application-x-generic");

        tree.connect_drag_data_received(Self::on_drag_data_received(m, c));
        tree.connect_drag_data_get(Self::on_drag_data_get(m));
        tree.connect_drag_motion(Self::on_drag_motion(
            &drag_hover_last_occurred,
            &drag_ends_last_occurred,
        ));

        tree.connect_drag_end(Self::on_drag_end(
            &drag_hover_last_occurred,
            &drag_ends_last_occurred,
        ));

        Self { cols, model, tree }
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
            popup.update(tree);
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
            if t.row_expanded(path) {
                t.collapse_row(path);
                return;
            }

            t.expand_to_path(path);
            let model = t.get_model().unwrap();
            let iter = model.get_iter(path).unwrap();

            if Self::iter_is_document(&model, &iter) {
                let iter_id = tree_iter_value!(model, &iter, 2, String);
                let iter_uuid = Uuid::parse_str(&iter_id).unwrap();
                m.send(Msg::OpenFile(Some(iter_uuid)));
            }
        })
    }

    fn on_drag_data_get(
        m: &Messenger,
    ) -> impl Fn(&TreeView, &DragContext, &SelectionData, u32, u32) {
        closure!(m => move |_, _, s, info, _| match info {
            LOCKBOOK_FILES_TARGET_INFO => {
                s.set(&s.get_target(), 8, &[]);
            }
            _ => m.send_err_dialog("Dragging data", progerr!("Unrecognized data format '{}'.", s.get_data_type().name())),
        })
    }

    fn on_drag_data_received(
        m: &Messenger,
        c: &Arc<LbCore>,
    ) -> impl Fn(&TreeView, &DragContext, i32, i32, &SelectionData, u32, u32) {
        closure!(m, c => move |w, d, x, y, s, info, time| {
            if let Some((Some(mut path), pos)) = w.get_dest_row_at_pos(x, y) {
                let model = w.get_model().unwrap().downcast::<TreeStore>().unwrap();

                let mut parent = model.get_iter(&path).unwrap();
                if (pos == TreeViewDropPosition::Before || pos == TreeViewDropPosition::After) || tree_iter_value!(model, &parent, 3, String) == format!("{:?}", FileType::Document) {
                    path.up();
                    parent = model.get_iter(&path).unwrap();
                }

                let parent_id = Uuid::parse_str(&tree_iter_value!(model, &parent, 2, String)).unwrap();

                match info {
                    LOCKBOOK_FILES_TARGET_INFO => {
                        let (paths, _) = w.get_selection().get_selected_rows();

                        for selected in paths {
                            let iter = model.get_iter(&selected).unwrap();
                            let id = Uuid::parse_str(&tree_iter_value!(model, &iter, 2, String)).unwrap();

                            match c.file_by_id(id) {
                                Ok(metadata) => if metadata.parent == parent_id {
                                    continue;
                                },
                                Err(err) => {
                                    m.send_err_dialog("getting metadata", err);
                                    continue;
                                }
                            };

                            match c.move_file(&id, parent_id) {
                                Ok(_) => {
                                    Self::move_iter(&model, &iter, &parent, true);
                                    model.remove(&iter);
                                }
                                Err(err) => m.send_err_dialog("moving", err)
                            }
                        }
                    }
                    URI_TARGET_INFO => {
                        let uris: Vec<String> = s.get_uris().iter().map(|uri| uri.to_string()).collect();
                        m.send(Msg::ShowDialogImportFile(parent_id, uris, None))
                    }
                    _ => m.send_err_dialog("Dropping data", progerr!("Unrecognized data format '{}'.", s.get_data_type().name())),
                }

                d.drop_finish(true, time);
            }
        })
    }

    fn on_drag_end(
        drag_hover_last_occurred: &Rc<RefCell<Option<u32>>>,
        drag_ends_last_occurred: &Rc<RefCell<Option<u32>>>,
    ) -> impl Fn(&TreeView, &DragContext) {
        closure!(drag_hover_last_occurred, drag_ends_last_occurred => move |_, _| {
            *drag_hover_last_occurred.borrow_mut() = None;
            *drag_ends_last_occurred.borrow_mut() = None;
        })
    }

    fn on_drag_motion(
        drag_hover_last_occurred: &Rc<RefCell<Option<u32>>>,
        drag_ends_last_occurred: &Rc<RefCell<Option<u32>>>,
    ) -> impl Fn(&TreeView, &DragContext, i32, i32, u32) -> GtkInhibit {
        closure!(drag_hover_last_occurred, drag_ends_last_occurred => move |w, d, x, y, time| {
            let vadj = w.get_vadjustment().unwrap();
            let height = w.get_allocated_height();

            if y > height - 50 || y < 50 { // is the drag cursor 50 pixels from the top or bottom
                *drag_ends_last_occurred.borrow_mut() = Some(time);
                timeout_add_local( // call a closure every 10 ms that will move the filetree up or down
                    10,
                    closure!(drag_ends_last_occurred, height, y => move || {
                        if let Some(t) = *drag_ends_last_occurred.borrow() {
                            if t == time {
                                if y > height - 50 { // move the filtree by an amount in respect to how close it is to the top or bottom
                                    vadj.set_value(vadj.get_value() + (y - (height - 50)) as f64);
                                } else if y < 50 {
                                    vadj.set_value(vadj.get_value() - (50 - y) as f64);
                                } else {
                                    panic!("impossible")
                                }

                                return Continue(true);
                            }
                        }
                        Continue(false)
                    }));
            } else {
                *drag_ends_last_occurred.borrow_mut() = None;
            }

            if let Some((Some(path), pos)) = w.get_dest_row_at_pos(x, y) {
                let model = w.get_model().unwrap();
                *drag_hover_last_occurred.borrow_mut() = Some(time);

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
                                    closure!(drag_hover_last_occurred, w, path => move || {
                                    if let Some(t) = *drag_hover_last_occurred.borrow() {
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
        let iter_icon = tree_iter_value!(model, iter, 0, String);
        let iter_name = tree_iter_value!(model, iter, 1, String);
        let iter_id = tree_iter_value!(model, iter, 2, String);
        let iter_ftype = tree_iter_value!(model, iter, 3, String);

        let new_parent = model.insert_with_values(
            Some(parent),
            None,
            &[0, 1, 2, 3],
            &[&iter_icon, &iter_name, &iter_id, &iter_ftype],
        );

        if let Some(it) = model.iter_children(Some(iter)) {
            Self::move_iter(model, &it, &new_parent, false);
        }

        if !is_at_top && model.iter_next(iter) {
            Self::move_iter(model, iter, parent, false);
        }
    }

    pub fn widget(&self) -> &GtkTreeView {
        &self.tree
    }

    pub fn selected_rows(&self) -> (Vec<GtkTreePath>, GtkTreeModel) {
        self.tree.get_selection().get_selected_rows()
    }

    pub fn populate_tree(
        &self,
        metadatas: &[DecryptedFileMetadata],
        parent_metadata: &DecryptedFileMetadata,
        parent_iter: &TreeIter,
    ) {
        let children: Vec<&DecryptedFileMetadata> = metadatas
            .iter()
            .filter(|metadata| {
                metadata.parent == parent_metadata.id && metadata.parent != metadata.id
            })
            .collect();

        for child in children {
            let item_iter = self.shallow_append(Some(parent_iter), child);

            if child.file_type == FileType::Folder {
                self.populate_tree(metadatas, child, &item_iter);
            }
        }
    }

    fn shallow_append(
        &self,
        iter: Option<&TreeIter>,
        metadata: &DecryptedFileMetadata,
    ) -> TreeIter {
        let icon_name = self.get_icon_name(&metadata.decrypted_name, &metadata.file_type);

        let name = &metadata.decrypted_name;
        let id = &metadata.id.to_string();
        let ftype = &format!("{:?}", metadata.file_type);
        self.model
            .insert_with_values(iter, None, &[0, 1, 2, 3], &[&icon_name, name, id, ftype])
    }

    pub fn fill(&self, root: &DecryptedFileMetadata, metadatas: &Vec<DecryptedFileMetadata>) {
        self.model.clear();
        let root_iter = self.shallow_append(None, root);
        self.populate_tree(metadatas.as_slice(), root, &root_iter);
    }

    pub fn refresh(&self, root: &DecryptedFileMetadata, metadatas: &Vec<DecryptedFileMetadata>) {
        let mut expanded_paths = Vec::<GtkTreePath>::new();
        self.search_expanded(&self.iter(), &mut expanded_paths);

        let sel = self.tree.get_selection();
        let (selected_paths, _) = sel.get_selected_rows();

        self.fill(root, metadatas);

        for path in expanded_paths {
            self.tree.expand_row(&path, false);
        }
        for path in selected_paths {
            sel.select_path(&path);
        }
    }

    pub fn add(&self, b: &LbCore, f: &DecryptedFileMetadata) -> LbResult<()> {
        let mut file = f.clone();
        let mut parent_iter: Option<GtkTreeIter>;
        loop {
            parent_iter = self.search(&self.iter(), &file.parent);
            if parent_iter.is_some() {
                break;
            }
            file = b.file_by_id(file.parent)?;
        }

        match parent_iter {
            Some(iter) => {
                self.shallow_append(Some(&iter), &file);
            }
            None => panic!("no parent found, should have at least found root!"),
        }

        self.select(&f.id);
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
        if let Some(it) = self.model.iter_children(Some(iter)) {
            if let Some(chit) = self.search(&it, id) {
                return Some(chit);
            }
        }
        if self.model.iter_next(iter) {
            if let Some(it) = self.search(iter, id) {
                return Some(it);
            }
        }
        None
    }

    pub fn search_expanded(&self, iter: &GtkTreeIter, expanded_paths: &mut Vec<GtkTreePath>) {
        let maybe_path = self.model.get_path(iter);

        if let Some(path) = maybe_path {
            if self.tree.row_expanded(&path) {
                expanded_paths.push(path);
            }
        }

        if let Some(it) = self.model.iter_children(Some(iter)) {
            self.search_expanded(&it, expanded_paths)
        }

        if self.model.iter_next(iter) {
            self.search_expanded(iter, expanded_paths)
        }
    }

    pub fn select(&self, id: &Uuid) {
        if let Some(it) = self.search(&self.iter(), id) {
            let p = self.model.get_path(&it).expect("could not get path");
            self.tree.expand_to_path(&p);

            let sel = &self.tree.get_selection();
            sel.unselect_all();
            sel.select_iter(&it);
        }
    }

    pub fn set_name(&self, id: &Uuid, name: &str) {
        if let Some(iter) = self.search(&self.iter(), id) {
            self.model.set(&iter, &[0], &[&name.to_string()]);
        }
    }

    pub fn remove(&self, id: &Uuid) {
        if let Some(iter) = self.search(&self.iter(), id) {
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
        let mut i = col.as_tree_store_index();
        while i >= 1 {
            i -= 1;
            let prev = self.cols.get(i as usize).unwrap();
            if self.tree_has_col(prev) {
                self.tree.insert_column(&col.as_tree_view_col(), i + 1);
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
                let iter = model.get_iter(tpath).unwrap();
                let iter_id = tree_iter_value!(model, &iter, 2, String);
                Some(Uuid::parse_str(&iter_id).unwrap())
            }
            None => None,
        }
    }

    pub fn iter_is_document(model: &GtkTreeModel, iter: &GtkTreeIter) -> bool {
        tree_iter_value!(model, iter, 3, String) == format!("{:?}", FileType::Document)
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

    fn as_tree_view_col(&self) -> GtkTreeViewColumn {
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
        c.add_attribute(&cell, attr, self.as_tree_store_index());

        c
    }

    fn as_tree_store_index(&self) -> i32 {
        *self as i32 + 1
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
enum PopupItem {
    NewDocument,
    NewFolder,
    Export,
    Rename,
    Open,
    Delete,
}

impl PopupItem {
    fn hashmap(m: &Messenger) -> HashMap<Self, GtkMenuItem> {
        let mut items = HashMap::new();
        for (item_key, action) in Self::data() {
            let name = if let PopupItem::NewFolder = item_key {
                "New Folder".to_string()
            } else if let PopupItem::NewDocument = item_key {
                "New Document".to_string()
            } else {
                format!("{:?}", item_key)
            };

            let mi = GtkMenuItem::with_label(&name);

            mi.connect_activate(closure!(m => move |_| m.send(action())));
            items.insert(item_key, mi);
        }
        items
    }

    #[rustfmt::skip]
    fn data() -> Vec<(Self, MsgFn)> {
        vec![
            (Self::NewDocument, || Msg::NewFile(FileType::Document)),
            (Self::NewFolder, || Msg::NewFile(FileType::Folder)),
            (Self::Export, || Msg::ShowDialogExportFile),
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
        let items = PopupItem::hashmap(m);
        let menu = GtkMenu::new();
        for (key, _) in &PopupItem::data() {
            menu.append(items.get(key).unwrap());
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
                (PopupItem::Export, true),
                (PopupItem::Rename, only_1 && !is_root),
                (PopupItem::Open, only_1),
                (PopupItem::Delete, at_least_1),
            ] {
                self.set_enabled(key, *is_enabled);
            }

            self.menu.show_all();
        }
    }

    fn set_enabled(&self, key: &PopupItem, condition: bool) {
        self.items.get(key).unwrap().set_sensitive(condition);
    }
}

const DELETE_KEY: u16 = 119;
