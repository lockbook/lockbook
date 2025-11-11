use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{mem, thread};

use egui::text_edit::TextEditState;
use egui::{
    Color32, Context, DragAndDrop, Event, EventFilter, Id, Key, LayerId, Modifiers, Order, Pos2,
    Rect, Sense, TextEdit, Ui, Vec2, WidgetText, vec2,
};
use egui_notify::Toasts;
use lb::Uuid;
use lb::blocking::Lb;
use lb::model::file::File;
use lb::model::file_metadata::FileType;
use lb::service::activity::RankingWeights;
use rfd::FileDialog;
use workspace_rs::show::DocType;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;

#[derive(Debug)]
pub struct FileTree {
    /// This is where the egui app caches files.
    pub files: Vec<File>,

    /// Set of selected files. To be selected, a file must be visible i.e. all its ancestors must be expanded. This is
    /// the set of files that will be deleted when the user presses the delete key, for example.
    pub selected: HashSet<Uuid>,

    /// Set of expanded files. To be expanded, a file must be a folder but need not be visible. A document is
    /// considered neither expanded nor collapsed.
    pub expanded: HashSet<Uuid>,

    /// Currently active file - if folder, this is where ctrl+n will add files, for example.
    pub cursor: Option<Uuid>,

    /// Files that have been marked with cmd + x can be moved to the cursored folder with cmd + v.
    pub cut: HashSet<Uuid>,

    /// Suggested files appear in a "folder" at the top of the tree.
    pub suggested_docs_folder_id: Uuid,
    pub suggested_docs: Arc<Mutex<Vec<Uuid>>>,

    /// Up to one file can be renamed at a time.
    pub rename_target: Option<Uuid>,
    pub rename_buffer: String,

    /// File export targets are selected asynchronously using the system file dialog.
    pub export: Arc<Mutex<Option<(File, PathBuf)>>>,

    /// Which file is the drag 'n' drop payload being hovered over and since when? Used to expand folders during dnd.
    pub drop: Option<(Uuid, Instant)>,

    /// Set to `true` and the cursor will be scrolled to on the next frame
    pub scroll_to_cursor: bool,
}

impl FileTree {
    pub fn new(files: Vec<File>) -> Self {
        Self {
            selected: Default::default(),
            expanded: [files.root()].into_iter().collect(),
            files,
            cursor: Default::default(),
            cut: Default::default(),
            suggested_docs_folder_id: Uuid::new_v4(),
            suggested_docs: Default::default(),
            rename_target: Default::default(),
            rename_buffer: Default::default(),
            export: Default::default(),
            drop: Default::default(),
            scroll_to_cursor: Default::default(),
        }
    }

    /// Updates the files in the tree. The selection and expansion are preserved.
    pub fn update_files(&mut self, files: Vec<File>) {
        self.files = files;
        self.expanded.retain(|&id| {
            self.files.iter().any(|f| f.id == id) || id == self.suggested_docs_folder_id
        });
        self.selected.retain(|&id| {
            self.files.iter().any(|f| f.id == id) || id == self.suggested_docs_folder_id
        });
        if let Some(cursor) = self.cursor {
            if !self.files.iter().any(|f| f.id == cursor) && cursor != self.suggested_docs_folder_id
            {
                self.cursor = Some(self.files.root());
            }
        }
    }

    /// Asynchronously recalculates the suggested files; requests repaint when complete.
    pub fn recalc_suggested_files(&mut self, core: &Lb, ctx: &egui::Context) {
        let core = core.clone();
        let suggested = self.suggested_docs.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let suggested_docs = core.suggested_docs(RankingWeights::default());
            match suggested_docs {
                Ok(docs) => {
                    let mut suggested = suggested.lock().unwrap();
                    *suggested = docs.into_iter().take(5).collect();
                }
                Err(err) => {
                    // todo: better error surfacing
                    println!("Failed to calculate suggested files: {err:?}");
                }
            }
            ctx.request_repaint();
        });
    }

    /// Expands `ids`. Does not select or deselect anything.
    fn expand(&mut self, ids: &[Uuid]) {
        self.expand_recursive(ids, Some(0));
    }

    /// Expands `ids` recursively to a maximum depth of `depth`. If `depth` is `None`, expands all the way. Does not
    /// select or deselect anything.
    fn expand_recursive(&mut self, ids: &[Uuid], depth: Option<usize>) {
        let ids = ids
            .iter()
            .copied()
            .filter(|&id| self.files.get_by_id(id).is_folder())
            .collect::<Vec<_>>();
        self.expanded.extend(ids.iter().copied());
        if depth == Some(0) {
            return;
        }
        for id in ids {
            let children = self
                .files
                .children(id)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            for child in children {
                self.expand_recursive(&[child.id], depth.map(|d| d - 1));
            }
        }
    }

    /// Expands nodes to increment the shortest distance from any id in `ids` to a collapsed descendent.
    fn expand_incremental(&mut self, ids: &[Uuid]) {
        for &id in ids {
            self.expand_recursive(ids, self.shortest_collapsed_distance(id));
        }
    }

    /// Helper that returns the shortest distance from `id` to a collapsed descendent.
    fn shortest_collapsed_distance(&self, id: Uuid) -> Option<usize> {
        if self.files.get_by_id(id).is_document() {
            return None;
        }
        if !self.expanded.contains(&id) {
            return Some(0);
        }
        let mut distance = None;
        for child in self.files.children(id) {
            let child_distance = self.shortest_collapsed_distance(child.id);
            distance = match (distance, child_distance) {
                (None, None) => None,
                (None, Some(child_distance)) => Some(child_distance + 1),
                (Some(distance), None) => Some(distance),
                (Some(distance), Some(child_distance)) => Some(distance.min(child_distance + 1)),
            };
        }
        distance
    }

    /// Collapses `ids`. Selections that are hidden are replaced with their closest visible ancestor.
    fn collapse(&mut self, ids: &[Uuid]) {
        self.expanded.retain(|&id| !ids.contains(&id));
        self.select_visible_ancestors();
    }

    /// Collapses all leaves under `ids`. Selections that are hidden are replaced with their closest visible ancestor.
    fn collapse_leaves(&mut self, ids: &[Uuid]) {
        let mut all_children = Vec::new();
        for &id in ids {
            let mut leaf_node = true; // guilty until proven innocent
            let children = self
                .files
                .children(id)
                .iter()
                .map(|f| f.id)
                .collect::<Vec<_>>();
            for child in &children {
                if self.expanded.contains(child) {
                    leaf_node = false; // sacrifice at least one child to live
                    break;
                }
            }
            if leaf_node {
                self.expanded.remove(&id); // else you will be collapsed
                self.select_visible_ancestors();
            }

            all_children.extend(children);
        }
        if !all_children.is_empty() {
            self.collapse_leaves(&all_children); // your descendants are cursed to repeat the cycle
        }
    }

    /// Helper that replaces each file in selection with its first visible ancestor (including itself). One option for
    /// making sure all selections are visible. See also `reveal_selection`.
    fn select_visible_ancestors(&mut self) {
        let selected = mem::take(&mut self.selected);
        for mut id in selected {
            while !self.is_visible(id) {
                id = self.files.get_by_id(id).parent;
            }
            self.selected.insert(id);
        }
    }

    /// Helper that expands the ancestors of the selected files. One option for making sure all selections are visible.
    /// See also `select_visible_ancestors`.
    pub fn reveal_selection(&mut self) {
        for mut id in self.selected.clone() {
            loop {
                if id == self.suggested_docs_folder_id {
                    break;
                }

                let parent = self.files.get_by_id(id).parent;
                if parent == id {
                    break;
                }

                self.expanded.insert(parent);
                id = parent;
            }
        }
    }

    /// Returns the file after id in depth-first order, folders first then alphabetically.
    fn next(&self, id: Uuid, visible_only: bool) -> Option<Uuid> {
        // if the file has children, return the first child
        // if `visible_only` is true then the child must be visible i.e. the file must be visible and expanded
        if !visible_only || (self.is_visible(id) && self.expanded.contains(&id)) {
            if let Some(first_child) = self.files.children(id).first() {
                return Some(first_child.id);
            }
        }

        // otherwise, return the next sibling of the file's closest ancestor (including itself) that has a next sibling
        let mut ancestor = id;
        loop {
            let parent = self.files.get_by_id(ancestor).parent;
            if !visible_only || self.is_visible(ancestor) {
                let siblings = self.files.children(parent);
                let mut found_file = false;
                for sibling in siblings {
                    if sibling.id == ancestor {
                        found_file = true;
                    } else if found_file {
                        return Some(sibling.id);
                    }
                }
            }

            if ancestor == parent {
                return None;
            }
            ancestor = parent;
        }
    }

    /// Returns the file before id in depth-first order, folders first then alphabetically.
    fn prev(&self, id: Uuid, visible_only: bool) -> Option<Uuid> {
        let parent = self.files.get_by_id(id).parent;
        if id == parent {
            return None;
        }

        let siblings = self.files.children(parent);
        let mut prev_sibling = None;
        let mut found_file = false;
        for sibling in siblings.into_iter().rev() {
            if sibling.id == id {
                found_file = true;
            } else if found_file {
                prev_sibling = Some(sibling.id);
                break;
            }
        }

        if let Some(prev_sibling) = prev_sibling {
            // if the file has a previous sibling, return the last descendent of the previous sibling
            // if `visible_only` is true then return the first visible ancestor of that descendent (including itself)
            let mut last_descendent = prev_sibling;
            loop {
                let children = self.files.children(last_descendent);
                if let Some(last_child) = children.last() {
                    last_descendent = last_child.id;
                } else {
                    break;
                }
            }
            if visible_only {
                loop {
                    if self.is_visible(last_descendent) {
                        break;
                    }
                    last_descendent = self.files.get_by_id(last_descendent).parent;
                }
            }
            Some(last_descendent)
        } else {
            // if the file is the first child of its parent, return the parent
            // if `visible_only` is true then return the first visible ancestor of the parent (including the parent)
            let mut ancestor = self.files.get_by_id(id).parent;
            if visible_only {
                loop {
                    if self.is_visible(ancestor) {
                        break;
                    }
                    ancestor = self.files.get_by_id(ancestor).parent;
                }
            }
            Some(ancestor)
        }
    }

    /// Returns the file after id in the order of suggested docs. Returns None if suggested docs are collapsed, if id
    /// is not suggested, or if there is no next suggested doc.
    fn next_suggested(&self, id: Uuid) -> Option<Uuid> {
        let Ok(suggested_docs) = self.suggested_docs.lock() else {
            return None;
        };

        if !self.expanded.contains(&self.suggested_docs_folder_id) {
            None // folder collapsed -> none
        } else if id == self.suggested_docs_folder_id {
            suggested_docs.first().copied() // folder -> first item
        } else {
            let idx = suggested_docs.iter().position(|&doc_id| doc_id == id)?;
            if idx + 1 < suggested_docs.len() {
                Some(suggested_docs[idx + 1]) // child -> next sibling
            } else {
                None // last child -> none
            }
        }
    }

    /// Returns the file before id in the order of suggested docs. Returns None if suggested docs are collapsed, if id
    /// is not suggested, or if there is no previous suggested doc.
    fn prev_suggested(&self, id: Uuid) -> Option<Uuid> {
        let Ok(suggested_docs) = self.suggested_docs.lock() else {
            return None;
        };

        if id == self.suggested_docs_folder_id {
            None // folder -> none
        } else if !self.expanded.contains(&self.suggested_docs_folder_id) {
            Some(self.suggested_docs_folder_id) // invisible item -> folder
        } else {
            let idx = suggested_docs.iter().position(|&doc_id| doc_id == id)?;
            if idx > 0 {
                Some(suggested_docs[idx - 1]) // child -> prev sibling
            } else {
                Some(self.suggested_docs_folder_id) // last child -> folder
            }
        }
    }

    /// A file is visible if all its ancestors are expanded.
    fn is_visible(&self, id: Uuid) -> bool {
        let file = self.files.get_by_id(id);
        if file.parent == file.id {
            return true;
        }
        self.expanded.contains(&file.parent) && self.is_visible(file.parent)
    }
}

#[derive(Debug, Default)]
pub struct Response {
    pub open_requests: HashSet<Uuid>,
    pub new_file: Option<bool>,
    pub new_drawing: Option<bool>,
    pub export_file: Option<(File, PathBuf)>,
    pub new_folder_modal: Option<File>,
    pub create_share_modal: Option<File>,
    pub move_requests: Vec<(Uuid, Uuid)>,
    pub rename_request: Option<(Uuid, String)>,
    pub delete_requests: HashSet<Uuid>,
    pub dropped_on: Option<Uuid>,
    pub space_inspector_root: Option<File>,
    pub clear_suggested: bool,
    pub clear_suggested_id: Option<Uuid>,
}

impl Response {
    fn union(self, other: Self) -> Self {
        let mut this = self;
        this.new_file = this.new_file.or(other.new_file);
        this.new_drawing = this.new_drawing.or(other.new_drawing);
        this.new_folder_modal = this.new_folder_modal.or(other.new_folder_modal);
        this.create_share_modal = this.create_share_modal.or(other.create_share_modal);
        this.export_file = this.export_file.or(other.export_file);
        this.open_requests.extend(other.open_requests);
        this.move_requests.extend(other.move_requests);
        this.rename_request = this.rename_request.or(other.rename_request);
        this.delete_requests.extend(other.delete_requests);
        this.dropped_on = this.dropped_on.or(other.dropped_on);
        this.space_inspector_root = this.space_inspector_root.or(other.space_inspector_root);
        this.clear_suggested = this.clear_suggested || other.clear_suggested;
        this.clear_suggested_id = this.clear_suggested_id.or(other.clear_suggested_id);
        this
    }
}

impl FileTree {
    pub fn show(&mut self, ui: &mut Ui, max_rect: Rect, toasts: &mut Toasts) -> Response {
        let mut resp = Response::default();
        let mut scroll_to_cursor = mem::take(&mut self.scroll_to_cursor);

        let full_doc_search_id = Id::from("full_doc_search");
        let suggested_docs_id = Id::from("suggested_docs");
        let file_tree_id = Id::from("file_tree");

        let tab_input = ui.input(|i| i.key_pressed(Key::Tab));

        if ui.memory(|m| m.has_focus(suggested_docs_id)) {
            // left arrow: collapse folder or move to folder (or surrender focus)
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowLeft)
                    || i.consume_key(Modifiers::NONE, Key::A)
            }) {
                if self.cursor == Some(self.suggested_docs_folder_id) {
                    if self.expanded.contains(&self.suggested_docs_folder_id) {
                        self.expanded.remove(&self.suggested_docs_folder_id);
                        self.selected.clear();
                    } else {
                        // focus to search
                        ui.memory_mut(|m| m.request_focus(full_doc_search_id));
                        self.selected.clear();
                        self.cursor = None;
                    }
                } else if self.cursor.is_some() {
                    self.cursor = Some(self.suggested_docs_folder_id);
                    self.selected.clear();
                }
            }

            // right arrow: expand folder or move to first child (or surrender focus)
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(Modifiers::NONE, Key::D)
            }) {
                if self.cursor == Some(self.suggested_docs_folder_id) {
                    if self.expanded.contains(&self.suggested_docs_folder_id) {
                        self.cursor = self.suggested_docs.lock().unwrap().first().copied();
                        self.selected.clear();
                        if let Some(cursor) = self.cursor {
                            self.selected.insert(cursor);
                        }
                    } else {
                        self.expanded.insert(self.suggested_docs_folder_id);
                    }
                } else if let Some(cursor) = self.cursor {
                    if let Some(next) = self.next_suggested(cursor) {
                        self.cursor = Some(next);
                        self.selected.clear();
                        self.selected.insert(next);
                    } else {
                        // focus to tree
                        ui.memory_mut(|m| m.request_focus(file_tree_id));
                        self.cursor = Some(self.files.root());
                        self.selected.clear();
                        self.selected.insert(self.files.root());
                    }
                }
            }

            // up arrow: move cursor up
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowUp)
                    || i.consume_key(Modifiers::NONE, Key::W)
                    || i.consume_key(Modifiers::SHIFT, Key::Tab)
            }) {
                if let Some(cursor) = self.cursor {
                    if let Some(prev) = self.prev_suggested(cursor) {
                        self.cursor = Some(prev);

                        if !ui.input(|i| i.raw.modifiers.shift) || tab_input {
                            self.selected.clear();
                        }
                        self.selected.insert(prev);
                    } else {
                        // focus to search
                        ui.memory_mut(|m| m.request_focus(full_doc_search_id));
                        self.selected.clear();
                        self.cursor = None;
                    }
                }
            }

            // down arrow: move cursor down
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(Modifiers::NONE, Key::S)
                    || i.consume_key(Modifiers::NONE, Key::Tab)
            }) {
                if let Some(cursor) = self.cursor {
                    if let Some(next) = self.next_suggested(cursor) {
                        self.cursor = Some(next);

                        if !ui.input(|i| i.raw.modifiers.shift) || tab_input {
                            self.selected.clear();
                        }
                        self.selected.insert(next);
                    } else {
                        // focus to tree
                        ui.memory_mut(|m| m.request_focus(file_tree_id));
                        self.cursor = Some(self.files.root());
                        self.selected.clear();
                    }
                }
            }
        } else if ui.memory(|m| m.has_focus(file_tree_id)) {
            // shift + left arrow: incremental recursive collapse
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::SHIFT, Key::ArrowLeft)
                    || i.consume_key(Modifiers::SHIFT, Key::A)
            }) {
                self.collapse_leaves(&Vec::from_iter(self.selected.iter().cloned()));
            }
            // left arrow: collapse selected or move selection to parent
            else if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowLeft)
                    || i.consume_key(Modifiers::NONE, Key::A)
            }) {
                scroll_to_cursor = true;

                // prefer to collapse all selected folders
                let mut collapsed_any = false;
                for id in self.selected.clone() {
                    if self.expanded.contains(&id) {
                        self.collapse(&[id]);
                        collapsed_any = true;
                    }
                }
                if let Some(cursor) = self.cursor {
                    if self.expanded.contains(&cursor) {
                        self.collapse(&[cursor]);
                        collapsed_any = true;
                    }
                }

                // if all selected folders are already collapsed, move selection to parent
                let mut selected_any_parents = false;
                if !collapsed_any {
                    let mut new_selection = HashSet::new();
                    for &id in &self.selected {
                        let parent = self.files.get_by_id(id).parent;
                        if id != parent {
                            selected_any_parents = true;
                        }
                        new_selection.insert(self.files.get_by_id(id).parent);
                    }
                    self.selected = new_selection;
                    if let Some(cursor) = self.cursor {
                        let parent = self.files.get_by_id(cursor).parent;
                        if cursor != parent {
                            selected_any_parents = true;
                        }
                        self.cursor = Some(self.files.get_by_id(cursor).parent);
                    }
                }

                if !collapsed_any && !selected_any_parents {
                    // focus to suggested
                    ui.memory_mut(|m| m.request_focus(suggested_docs_id));
                    self.selected.clear();
                    self.cut.clear();
                    self.cursor = if self.expanded.contains(&self.suggested_docs_folder_id) {
                        self.suggested_docs.lock().unwrap().last().copied()
                    } else {
                        Some(self.suggested_docs_folder_id)
                    };
                }
            }

            // shift + right arrow: incremental recursive expand
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::SHIFT, Key::ArrowRight)
                    || i.consume_key(Modifiers::SHIFT, Key::D)
            }) {
                self.expand_incremental(&Vec::from_iter(self.selected.clone()));
            }
            // right arrow: expand selected or move selection to first child
            else if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(Modifiers::NONE, Key::D)
            }) {
                scroll_to_cursor = true;

                // prefer to expand all selected folders
                let mut expanded_any = false;
                for id in self.selected.clone() {
                    if self.files.get_by_id(id).is_folder() && !self.expanded.contains(&id) {
                        self.expand(&[id]);
                        expanded_any = true;
                    }
                }
                if let Some(cursor) = self.cursor {
                    if self.files.get_by_id(cursor).is_folder() && !self.expanded.contains(&cursor)
                    {
                        self.expand(&[cursor]);
                        expanded_any = true;
                    }
                }

                // if all selected folders are already expanded, move selection to first child
                let mut new_selection = self.selected.clone();
                let mut new_cursor = self.cursor;
                let mut advanced_to_children = false;
                if !expanded_any {
                    new_selection.clear();
                    for &id in &self.selected {
                        let mut advanced_to_child = false;
                        if let Some(next) = self.next(id, false) {
                            if self.files.children(id).iter().any(|f| f.id == next) {
                                new_selection.insert(next);
                                advanced_to_child = true;
                                advanced_to_children = true;
                            }
                        }
                        if !advanced_to_child {
                            new_selection.insert(id); // no children -> leave alone
                        }
                    }
                    if let Some(cursor) = self.cursor {
                        if let Some(next) = self.next(cursor, false) {
                            if self.files.children(cursor).iter().any(|f| f.id == next) {
                                new_cursor = Some(next);
                                advanced_to_children = true;
                            }
                        }
                    }
                }

                // if no children, move selection to next sibling within respective parents
                let mut advanced_to_siblings = false;
                if !advanced_to_children {
                    new_selection.clear();
                    for &id in &self.selected {
                        let file = self.files.get_by_id(id);
                        let mut advanced_to_sibling = false;
                        if let Some(next) = self.next(id, false) {
                            if self
                                .files
                                .children(file.parent)
                                .iter()
                                .any(|f| f.id == next)
                            {
                                new_selection.insert(next);
                                advanced_to_sibling = true;
                                advanced_to_siblings = true;
                            }
                        }
                        if !advanced_to_sibling {
                            new_selection.insert(id); // no further siblings -> leave alone
                        }
                    }
                    if let Some(cursor) = self.cursor {
                        let file = self.files.get_by_id(cursor);
                        if let Some(next) = self.next(cursor, false) {
                            if self
                                .files
                                .children(file.parent)
                                .iter()
                                .any(|f| f.id == next)
                            {
                                new_cursor = Some(next);
                                advanced_to_siblings = true;
                            }
                        }
                    }
                }

                // finally, if none of the above, advance to sibling of containing folder
                if !advanced_to_children && !advanced_to_siblings {
                    new_selection.clear();
                    for &id in &self.selected {
                        let file = self.files.get_by_id(id);
                        let parent = self.files.get_by_id(file.parent);
                        let mut advanced_to_parent_sibling = false;
                        if let Some(next) = self.next(id, false) {
                            if self
                                .files
                                .children(parent.parent)
                                .iter()
                                .any(|f| f.id == next)
                            {
                                new_selection.insert(next);
                                advanced_to_parent_sibling = true;
                            }
                        }
                        if !advanced_to_parent_sibling {
                            new_selection.insert(id); // no further siblings -> leave alone
                        }
                    }
                    if let Some(cursor) = self.cursor {
                        let file = self.files.get_by_id(cursor);
                        let parent = self.files.get_by_id(file.parent);
                        if let Some(next) = self.next(cursor, false) {
                            if self
                                .files
                                .children(parent.parent)
                                .iter()
                                .any(|f| f.id == next)
                            {
                                new_cursor = Some(next);
                            }
                        }
                    }
                }

                self.selected = new_selection;
                self.reveal_selection();
                self.cursor = new_cursor;
            }

            // up arrow: move selection to previous visible node (or surrender focus)
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowUp)
                    || i.consume_key(Modifiers::NONE, Key::W)
                    || i.consume_key(Modifiers::SHIFT, Key::Tab)
            }) {
                scroll_to_cursor = true;

                if let Some(cursor) = self.cursor {
                    if let Some(prev) = self.prev(cursor, true) {
                        self.cursor = Some(prev);

                        if !ui.input(|i| i.raw.modifiers.shift) || tab_input {
                            self.selected.clear();
                        }
                        self.selected.insert(prev);
                    } else {
                        // focus to suggested
                        ui.memory_mut(|m| m.request_focus(suggested_docs_id));
                        self.cursor = if self.expanded.contains(&self.suggested_docs_folder_id) {
                            self.suggested_docs.lock().unwrap().last().copied()
                        } else {
                            Some(self.suggested_docs_folder_id)
                        };
                    }
                }
            }

            // down arrow: move selection to next visible node
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(Modifiers::NONE, Key::S)
                    || i.consume_key(Modifiers::NONE, Key::Tab)
            }) {
                scroll_to_cursor = true;

                if let Some(cursor) = self.cursor {
                    if let Some(next) = self.next(cursor, true) {
                        self.cursor = Some(next);

                        if !ui.input(|i| i.raw.modifiers.shift) || tab_input {
                            self.selected.clear();
                        }
                        self.selected.insert(next);
                    }
                }
            }

            // cmd + x: cut selected files
            if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::X))
                || ui.input(|i| i.events.contains(&Event::Cut))
            {
                self.cut = self.selected.clone();
                if let Some(cursor) = self.cursor {
                    self.cut.insert(cursor);
                }
            }

            // cmd + v: paste clipped files into cursor location
            if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::V))
                || ui.input(|i| i.events.iter().any(|e| matches!(e, &Event::Paste(_))))
            {
                if let Some(cursor) = self.cursor {
                    let cursor_file = self.files.get_by_id(cursor);
                    let dest = if cursor_file.is_folder() { cursor } else { cursor_file.parent };
                    for id in mem::take(&mut self.cut) {
                        resp.move_requests.push((id, dest));
                    }
                }
            }

            // cmd + r: rename cursored file
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::COMMAND, Key::R) || i.consume_key(Modifiers::NONE, Key::F2)
            }) {
                if let Some(cursor) = self.cursor {
                    self.init_rename(ui.ctx(), cursor);
                }
            }

            // cmd + a: select all files in folder containing cursor
            if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::A)) {
                if let Some(cursor) = self.cursor {
                    let parent = self.files.get_by_id(cursor).parent;
                    self.selected.clear();
                    for file in self.files.children(parent) {
                        self.selected.insert(file.id);
                    }
                }
            }

            // backspace or delete: delete selected files
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace))
                || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete))
            {
                for id in &self.selected {
                    resp.delete_requests.insert(*id);
                }
            }
        }

        if ui.memory(|m| m.has_focus(suggested_docs_id) || m.has_focus(file_tree_id)) {
            // enter/space: open selected files or toggle folder expansion
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::Enter)
                    || i.consume_key(Modifiers::NONE, Key::Space)
            }) {
                let mut documents = Vec::new();
                let mut collapsed_folders = Vec::new();
                let mut expanded_folders = Vec::new();

                for &id in self.selected.iter().chain(self.cursor.iter()) {
                    if id == self.suggested_docs_folder_id {
                        if self.expanded.contains(&id) {
                            expanded_folders.push(id);
                        } else {
                            collapsed_folders.push(id);
                        }
                    } else if self.files.get_by_id(id).is_document() {
                        documents.push(id);
                    } else if self.expanded.contains(&id) {
                        expanded_folders.push(id);
                    } else {
                        collapsed_folders.push(id);
                    }
                }

                if !documents.is_empty() {
                    resp.open_requests.extend(documents);
                } else if !collapsed_folders.is_empty() {
                    self.expanded.extend(collapsed_folders);
                } else {
                    self.expanded.retain(|id| !expanded_folders.contains(id));
                }
            }
        }

        if !ui.memory(|m| m.has_focus(file_tree_id)) {
            self.cut.clear();
        }

        resp
            // show suggested docs
            .union(ui.vertical(|ui| self.show_suggested(ui)).inner)
            // show file tree
            .union({
                ui.vertical(|ui| {
                    self.show_recursive(ui, toasts, self.files.root(), 0, scroll_to_cursor)
                })
                .inner
                .union(self.show_padding(ui, toasts, max_rect))
            })
    }

    fn show_suggested(&mut self, ui: &mut Ui) -> Response {
        let mut resp = Response::default();

        let suggested_docs_id = Id::new("suggested_docs");
        let focused = ui.memory(|m| m.has_focus(suggested_docs_id));
        let suggested_docs = {
            let Ok(suggested_docs) = self.suggested_docs.lock() else {
                return resp;
            };
            suggested_docs.clone()
        };

        // suggested "folder"
        let is_expanded = self.expanded.contains(&self.suggested_docs_folder_id);
        let is_cursored = self.cursor == Some(self.suggested_docs_folder_id);
        let mut default_fill = ui.style().visuals.extreme_bg_color;

        if focused && is_cursored {
            default_fill = ui.style().visuals.selection.bg_fill;
        }

        let suggested_docs_btn = Button::default()
            .icon(&Icon::FOLDER.size(19.0))
            .icon_color(ui.style().visuals.widgets.active.bg_fill)
            .text("Suggested Documents")
            .default_fill(default_fill)
            .frame(true)
            .hexpand(true)
            .padding(vec2(15., 7.))
            .show(ui);

        suggested_docs_btn.context_menu(|ui| {
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                ui.close_menu();
            }

            if ui.button("Clear All").clicked() {
                resp.clear_suggested = true;
                ui.close_menu();
            }
        });

        if suggested_docs_btn.clicked() {
            ui.memory_mut(|m| m.request_focus(suggested_docs_id));
            self.selected.clear();
            self.cut.clear();
            self.cursor = Some(self.suggested_docs_folder_id);

            if is_expanded {
                self.expanded.remove(&self.suggested_docs_folder_id);
            } else {
                self.expanded.insert(self.suggested_docs_folder_id);
            }
        }

        // suggested docs
        if is_expanded {
            for &id in &suggested_docs {
                let file = self.files.get_by_id(id);
                let is_selected = self.selected.contains(&id);
                let is_cursored = self.cursor == Some(id);

                let mut text = WidgetText::from(&file.name);
                let mut default_fill = ui.style().visuals.extreme_bg_color;
                if is_selected && focused && !is_cursored {
                    text = text.color(ui.style().visuals.widgets.active.bg_fill);
                }
                if is_cursored && focused {
                    default_fill = ui.style().visuals.selection.bg_fill
                }

                let icon = DocType::from_name(&file.name).to_icon();
                let file_resp = Button::default()
                    .icon(&icon)
                    .text(text)
                    .default_fill(default_fill)
                    .frame(true)
                    .hexpand(true)
                    .indent(15.)
                    .show(ui);

                file_resp.context_menu(|ui| {
                    if ui.button("Remove Suggestion").clicked() {
                        resp.clear_suggested_id = Some(id);
                        ui.close_menu();
                    }
                });

                if file_resp.clicked() {
                    ui.memory_mut(|m| m.surrender_focus(suggested_docs_id));
                    self.selected.clear();
                    self.cut.clear();
                    self.cursor = Some(self.suggested_docs_folder_id);

                    resp.open_requests.insert(id);
                }
            }
        }

        resp
    }

    fn show_recursive(
        &mut self, ui: &mut Ui, toasts: &mut Toasts, id: Uuid, depth: usize, scroll_to_cursor: bool,
    ) -> Response {
        let mut resp = Response::default();

        let file = self.files.get_by_id(id).clone();

        let is_selected = self.selected.contains(&id);
        let is_expanded = self.expanded.contains(&id);
        let is_cursored = self.cursor == Some(id);
        let is_cut = self.cut.contains(&id);
        let is_renaming = self.rename_target == Some(id);
        let indent = depth as f32 * 15.;

        let btn_margin = egui::vec2(10.0, 0.0);
        let btn_rounding = 5.0;

        let file_tree_id = Id::new("file_tree");
        let focused = ui.memory(|m| m.has_focus(file_tree_id));

        let doc_type = DocType::from_name(&file.name);
        let mut text = if doc_type.hide_ext() {
            let wo = Path::new(&file.name)
                .file_stem()
                .map(|stem| stem.to_str().unwrap())
                .unwrap_or(&file.name);
            WidgetText::from(wo)
        } else {
            WidgetText::from(&file.name)
        };
        let mut default_fill = ui.style().visuals.extreme_bg_color;
        if is_selected {
            default_fill = ui.visuals().code_bg_color;

            ui.visuals_mut().widgets.hovered.bg_fill =
                default_fill.lerp_to_gamma(ui.visuals().text_color(), 0.1);
        } else {
            ui.visuals_mut().widgets.hovered.bg_fill = ui
                .visuals()
                .code_bg_color
                .linear_multiply(if ui.visuals().dark_mode { 0.1 } else { 0.9 });
        }

        if is_cursored && focused {
            default_fill = ui.style().visuals.selection.bg_fill;
        }
        if is_cut {
            text = text.strikethrough();
        }

        // renaming
        if is_renaming {
            ui.spacing_mut().indent = indent;
            ui.visuals_mut().indent_has_left_vline = false;
            let rename_resp = ui
                .indent("rename_file_indent", |ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.rename_buffer)
                            .frame(false)
                            .margin(ui.spacing().button_padding + btn_margin)
                            .id(Id::new("rename_file")),
                    )
                })
                .inner;

            ui.painter().rect_stroke(
                rename_resp.rect.expand(5.0),
                btn_rounding,
                egui::Stroke::new(1.0, ui.style().visuals.widgets.active.bg_fill),
            );

            if !rename_resp.has_focus() && !rename_resp.lost_focus() {
                // request focus on the first frame (todo: wrong but works)
                rename_resp.request_focus();
            }
            if rename_resp.has_focus() {
                // focus lock filter must be set every frame
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        rename_resp.id,
                        EventFilter {
                            tab: true, // suppress 'tab' behavior
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: false, // press 'esc' to release focus
                        },
                    )
                })
            }

            // submit
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                resp.rename_request = Some((id, self.rename_buffer.clone()));
                self.rename_target = None;
            }

            // release focus to cancel ('esc' or click elsewhere)
            if rename_resp.lost_focus() {
                self.rename_target = None;
            }

            return resp; // note: early return
        }

        // render
        let button = Button::default()
            .text(text)
            .default_fill(default_fill)
            .rounding(btn_rounding)
            .margin(btn_margin)
            .frame(true)
            .hexpand(true)
            .indent(indent)
            .padding(vec2(15., 7.));

        let icon_size = 19.0;

        let file_resp = if file.is_document() {
            let icon = doc_type.to_icon().size(icon_size);
            let file_resp = button.icon(&icon).icon_color(ui.style().visuals.text_color().linear_multiply(0.5)).show(ui);

            file_resp
        } else {
            let is_shared = !file.shares.is_empty();

            let icon = if is_expanded {
                Icon::FOLDER_OPEN
            } else if is_shared {
                Icon::SHARED_FOLDER
            } else {
                Icon::FOLDER
            }.size(icon_size);

            let file_resp = button
                .icon(&icon)
                .icon_color(ui.style().visuals.widgets.active.bg_fill)
                .show(ui);
            if is_expanded {
                resp = resp.union(self.show_children_recursive(
                    ui,
                    toasts,
                    id,
                    depth + 1,
                    scroll_to_cursor,
                ));
            };

            file_resp
        };

        // init rename
        if file_resp.double_clicked() {
            self.init_rename(ui.ctx(), file.id);
        }
        // select
        else if file_resp.clicked() {
            let mut shift_clicked = false;
            if let Some(cursored_file) = self.cursor {
                // shift-click to add visible files between cursor and target to selection
                if ui.input(|i| i.raw.modifiers.shift) {
                    shift_clicked = true;

                    // inefficient but works
                    let mut selected_down = false;
                    let mut inbetween_file = cursored_file;
                    let mut selection = Vec::new();
                    loop {
                        selection.push(inbetween_file);
                        if inbetween_file == id {
                            selected_down = true;
                            break;
                        }
                        if let Some(next_file) = self.next(inbetween_file, true) {
                            inbetween_file = next_file;
                        } else {
                            break;
                        }
                    }
                    if !selected_down {
                        // user must have shift-clicked a file above the cursor; try again in reverse
                        selection.clear();
                        inbetween_file = cursored_file;
                        loop {
                            selection.push(inbetween_file);
                            if inbetween_file == id {
                                break;
                            }
                            if let Some(prev_file) = self.prev(inbetween_file, true) {
                                inbetween_file = prev_file;
                            } else {
                                break;
                            }
                        }
                    }
                    self.selected.extend(selection);
                }
            }

            let mut cmd_clicked = false;
            if !shift_clicked && ui.input(|i| i.raw.modifiers.command) {
                cmd_clicked = true;

                self.selected.insert(id);
            }

            if !shift_clicked && !cmd_clicked {
                self.selected.clear();
                self.selected.insert(id);

                if file.is_document() {
                    resp.open_requests.insert(id);
                } else if !is_expanded {
                    self.expand(&[id]);
                } else {
                    self.collapse(&[id]);
                }

                ui.memory_mut(|m| m.surrender_focus(file_tree_id));
            } else {
                ui.memory_mut(|m| m.request_focus(file_tree_id));
            }

            self.cut.clear();
            self.cursor = Some(id);
            ui.ctx().request_repaint();
        }

        // context menu
        let mut context_menu_resp = Response::default();
        file_resp.context_menu(|ui| {
            context_menu_resp = self.context_menu(ui, toasts, Some(id));
        });
        resp = resp.union(context_menu_resp);

        // file export
        let mut export = self.export.lock().unwrap();
        if export.is_some() {
            resp.export_file = export.clone();
            *export = None;
        }
        mem::drop(export);

        // drag 'n' drop:
        // when drag starts, dragged file sets dnd payload
        if file_resp.dragged()
            && ui.input(|i| {
                let (Some(pos), Some(origin)) =
                    (i.pointer.interact_pos(), i.pointer.press_origin())
                else {
                    return false;
                };

                pos.distance(origin) > 8. // egui's drag detection is too sensitive and not configurable
            })
            // must not be already dragging something else
            && !DragAndDrop::has_any_payload(ui.ctx())
        {
            DragAndDrop::set_payload(ui.ctx(), id);

            // the selection is what's actually moved
            // dragging a selected file moves all selected files
            // dragging an unselected file moves only that file (and clears the selection)
            if !self.selected.contains(&id) {
                self.selected.clear();
                self.selected.insert(id);
            }
        }

        let file = self.files.get_by_id(id);
        // during drag, drop target renders indicator
        let mut hovering_file_center = false;
        if let (Some(pointer), true) =
            (ui.input(|i| i.pointer.interact_pos()), DragAndDrop::has_any_payload(ui.ctx()))
        {
            let contains_pointer = file_resp.rect.contains(pointer);
            if contains_pointer
            // ^ you'd think this would always be true (some suffering occurred here)
            // either something is deeply wrong with egui or something is deeply wrong with me
            {
                // awkwardly we can't use the drag 'n' drop state to adjust how the button is rendered because we don't
                // have the response until after we draw it, so we'll just draw something on top of it instead
                let stroke = ui.style().visuals.widgets.active.fg_stroke;
                hovering_file_center =
                    (pointer.y - file_resp.rect.center().y).abs() < file_resp.rect.height() / 4.;
                if file.is_folder() && hovering_file_center {
                    // drop into hovered folder (indicated by a rectangle)
                    ui.with_layer_id(
                        LayerId::new(Order::PanelResizeLine, Id::from("file_tree_drop_indicator")),
                        |ui| {
                            ui.painter()
                                .rect(file_resp.rect, 2., Color32::TRANSPARENT, stroke);
                        },
                    );

                    // scroll so that target is visible
                    ui.scroll_to_rect(file_resp.rect, None);
                } else {
                    // drop as sibling to hovered file (indicated by a line)
                    let y = if pointer.y < file_resp.rect.center().y {
                        file_resp.rect.min.y
                    } else {
                        file_resp.rect.max.y
                    };
                    let mut x_range = file_resp.rect.x_range();
                    x_range.min += indent;

                    ui.with_layer_id(
                        LayerId::new(Order::PanelResizeLine, Id::from("file_tree_drop_indicator")),
                        |ui| {
                            ui.painter().hline(x_range, y, stroke);
                            ui.painter().circle_filled(
                                Pos2 { x: x_range.min, y },
                                3.,
                                stroke.color,
                            );
                        },
                    );

                    // scroll so that targets on both sides of the line are visible
                    if pointer.y < file_resp.rect.center().y {
                        if self.prev(file.id, true).is_some() {
                            // scroll to reveal target above and self
                            let mut rect = file_resp.rect;
                            rect.min.y -= rect.height();
                            ui.scroll_to_rect(rect, None);
                        }
                    } else if self.next(file.id, true).is_some() {
                        // scroll to reveal target below and self
                        let mut rect = file_resp.rect;
                        rect.max.y += rect.height();
                        ui.scroll_to_rect(rect, None);
                    };
                }
            }

            // during drag, drop target expands after debounce if folder
            if file.is_folder() && hovering_file_center {
                if let Some((drop_id, drop_start)) = self.drop.as_mut() {
                    if !contains_pointer {
                        self.drop = None; // pointer left
                    } else if *drop_id != id {
                        *drop_id = id;
                        *drop_start = Instant::now(); // drop target changed
                    } else if drop_start.elapsed() > Duration::from_millis(600) {
                        self.expanded.insert(id); // expand after debounce
                    }
                } else if contains_pointer {
                    self.drop = Some((id, Instant::now())); // pointer entered
                }
            }
        }

        // when drag ends, dragged file clears drag state
        if file_resp.drag_stopped() {
            self.drop = None;
        }
        // when drag ends, dropped-on file consumes dnd payload and emits move operation
        // the dnd payload itself is ignored because we always move the selection
        if file_resp.dnd_release_payload::<Uuid>().is_some() {
            let destination =
                if file.is_folder() && hovering_file_center { id } else { file.parent };
            for &selected in &self.selected {
                resp.move_requests.push((selected, destination));
            }
        }

        if is_cursored && scroll_to_cursor {
            // todo: sometimes this doesn't scroll far enough to actually reveal the rect
            // it works more reliably when the usage/nav/sync panel is commented out
            // perhaps egui has a bug related to how we're mixing top-down and bottom-up layouts
            ui.scroll_to_rect(file_resp.rect, None);
        }

        resp
    }

    fn show_children_recursive(
        &mut self, ui: &mut Ui, toasts: &mut Toasts, id: Uuid, depth: usize, scroll_to_cursor: bool,
    ) -> Response {
        let children_ids = self
            .files
            .children(id)
            .iter()
            .map(|f| f.id)
            .collect::<Vec<_>>();
        let mut resp = Response::default();
        for child in children_ids {
            resp = resp.union(self.show_recursive(ui, toasts, child, depth, scroll_to_cursor));
        }
        resp
    }

    fn show_padding(&mut self, ui: &mut Ui, toasts: &mut Toasts, max_rect: Rect) -> Response {
        let mut resp = Response::default();

        let mut desired_size = Vec2::new(max_rect.width(), 0.);
        let min_rect = ui.min_rect();
        desired_size.y = if min_rect.height() < max_rect.height() {
            // fill available space
            max_rect.height() - min_rect.height()
        } else {
            0.
        };
        let padding_resp = ui.allocate_response(desired_size, Sense::click());

        // context menu
        let mut context_menu_resp = Response::default();
        padding_resp.context_menu(|ui| {
            context_menu_resp = self.context_menu(ui, toasts, None);
        });
        resp = resp.union(context_menu_resp);

        // during drag, render indicator
        if let (Some(pointer), Some(_)) =
            (ui.input(|i| i.pointer.interact_pos()), padding_resp.dnd_hover_payload::<Uuid>())
        {
            if padding_resp.rect.contains(pointer)
            // ^ you'd think this would always be true (some suffering occurred here)
            // either something is deeply wrong with egui or something is deeply wrong with me
            {
                let stroke = ui.style().visuals.widgets.active.fg_stroke;
                ui.with_layer_id(
                    LayerId::new(Order::PanelResizeLine, Id::from("file_tree_drop_indicator")),
                    |ui| {
                        ui.painter()
                            .rect(padding_resp.rect, 2., Color32::TRANSPARENT, stroke);
                    },
                );
            }
        }

        // when drag ends, consume dnd payload and emit move operation
        // the dnd payload itself is ignored because we always move the selection
        if padding_resp.dnd_release_payload::<Uuid>().is_some() {
            for &selected in &self.selected {
                resp.move_requests.push((selected, self.files.root()));
            }
        }

        resp
    }

    fn context_menu(
        &mut self, ui: &mut egui::Ui, toasts: &mut Toasts, file: Option<Uuid>,
    ) -> Response {
        let mut resp = Response::default();
        ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);

        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.close_menu();
        }

        if ui.button("New Document").clicked() {
            resp.new_file = Some(true);
            ui.close_menu();
        }

        if ui.button("New Drawing").clicked() {
            resp.new_drawing = Some(true);
            ui.close_menu();
        }

        if ui.button("New Folder").clicked() {
            let file = if let Some(file) = file { file } else { self.files.root() };
            resp.new_folder_modal = Some(self.files.get_by_id(file).clone());
            ui.close_menu();
        }

        if let Some(file) = file {
            ui.separator();

            if ui.button("Rename").clicked() {
                self.init_rename(ui.ctx(), file);
                ui.close_menu();
            }

            if ui.button("Delete").clicked() {
                if self.selected.contains(&file) {
                    resp.delete_requests.extend(&self.selected);
                } else {
                    resp.delete_requests.insert(file);
                }
                ui.close_menu();
            }

            ui.separator();

            if ui.button("Export").clicked() {
                let file = self.files.get_by_id(file).clone();
                let export = self.export.clone();
                let ctx = ui.ctx().clone();

                thread::spawn(move || {
                    if let Some(folder) = FileDialog::new().pick_folder() {
                        let mut export = export.lock().unwrap();
                        *export = Some((file, folder.clone()));
                    }
                    ctx.request_repaint();
                });
                ui.close_menu();
            }

            if ui.button("Share").clicked() {
                let file = self.files.get_by_id(file).clone();
                resp.create_share_modal = Some(file);
                ui.close_menu();
            }

            if ui.button("Copy Link").clicked() {
                ui.ctx()
                    .output_mut(|o| o.copied_text = format!("lb://{file}"));
                toasts.success("Copied link!");
                ui.close_menu();
            }

            let file = self.files.get_by_id(file).clone();
            if file.is_folder() {
                ui.separator();
                if ui.button("Space Inspector").clicked() {
                    resp.space_inspector_root = Some(file);
                    ui.close_menu();
                }
            }
        }
        resp
    }

    fn init_rename(&mut self, ctx: &Context, file: Uuid) {
        let file = self.files.get_by_id(file);

        self.rename_target = Some(file.id);
        self.rename_buffer = file.name.clone();

        let name = &self.rename_buffer;
        let end_pos = name.rfind('.').unwrap_or(name.len());

        let mut rename_edit_state = TextEditState::default();
        rename_edit_state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange {
                primary: egui::text::CCursor::new(end_pos),
                secondary: egui::text::CCursor::new(0),
            }));
        TextEdit::store_state(ctx, Id::new("rename_file"), rename_edit_state);
    }
}

pub trait FilesExt {
    fn root(&self) -> Uuid;
    fn get_by_id(&self, id: Uuid) -> &File;
    fn children(&self, id: Uuid) -> Vec<&File>;
}

impl FilesExt for [File] {
    fn root(&self) -> Uuid {
        for file in self {
            if file.parent == file.id {
                return file.id;
            }
        }
        unreachable!("unable to find root in metadata list")
    }

    fn get_by_id(&self, id: Uuid) -> &File {
        if let Some(file) = self.iter().find(|f| f.id == id) {
            file
        } else {
            unreachable!("unable to find file with id: {:?}", id)
        }
    }

    fn children(&self, id: Uuid) -> Vec<&File> {
        let mut children: Vec<_> = self
            .iter()
            .filter(|f| f.parent == id && f.parent != f.id)
            .collect();
        children.sort_by(|a, b| match (a.file_type, b.file_type) {
            (FileType::Folder, FileType::Document) => Ordering::Less,
            (FileType::Document, FileType::Folder) => Ordering::Greater,
            (_, _) => a.name.cmp(&b.name),
        });
        children
    }
}

#[cfg(test)]
mod test {
    use lb::Uuid;
    use lb::model::file::File;
    use lb::model::file_metadata::FileType;

    use super::FileTree;

    #[test]
    fn select_deselect() {
        /*
         * 0
         * ├── 1
         * │   ├── 2
         * │   └── 3
         * └── 4
         */
        let ids =
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let files = vec![
            file(0, 0, FileType::Folder, &ids),
            file(1, 0, FileType::Folder, &ids),
            file(2, 1, FileType::Document, &ids),
            file(3, 1, FileType::Document, &ids),
            file(4, 0, FileType::Document, &ids),
        ];

        let mut tree = FileTree::new(files);

        assert_eq!(tree.selected, vec![].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.selected.insert(ids[1]);
        tree.reveal_selection();

        assert_eq!(tree.selected, vec![ids[1]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.selected.insert(ids[1]);
        tree.selected.insert(ids[2]);
        tree.selected.insert(ids[3]);
        tree.reveal_selection();

        assert_eq!(tree.selected, vec![ids[1], ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.selected.remove(&ids[1]);

        assert_eq!(tree.selected, vec![ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.selected.clear();

        assert_eq!(tree.selected, vec![].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());
    }

    #[test]
    fn collapse_expand() {
        /*
         * 0
         * ├── 1
         * │   ├── 2
         * │   └── 3
         * └── 4
         */
        let ids =
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let files = vec![
            file(0, 0, FileType::Folder, &ids),
            file(1, 0, FileType::Folder, &ids),
            file(2, 1, FileType::Document, &ids),
            file(3, 1, FileType::Document, &ids),
            file(4, 0, FileType::Document, &ids),
        ];

        let mut tree = FileTree::new(files);

        assert_eq!(tree.selected, vec![].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.collapse(&[ids[0]]);
        tree.selected.insert(ids[0]);
        tree.reveal_selection();

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![].into_iter().collect());

        tree.expand(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.expand_recursive(&[ids[0]], None);
        tree.selected.clear();
        tree.selected.insert(ids[2]);
        tree.selected.insert(ids[3]);
        tree.reveal_selection();

        assert_eq!(tree.selected, vec![ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.collapse(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[1]].into_iter().collect());
    }

    #[test]
    fn collapse_leafs_expand_incremental() {
        /*
         * 0
         * ├── 1
         * ├── 2
         * │   └── 3
         * └── 4
         *     ├── 5
         *     └── 6
         *         └── 7
         */
        let ids = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let files = vec![
            file(0, 0, FileType::Folder, &ids),
            file(1, 0, FileType::Document, &ids),
            file(2, 0, FileType::Folder, &ids),
            file(3, 2, FileType::Document, &ids),
            file(4, 0, FileType::Folder, &ids),
            file(5, 4, FileType::Document, &ids),
            file(6, 4, FileType::Folder, &ids),
            file(7, 6, FileType::Document, &ids),
        ];

        let mut tree = FileTree::new(files);

        assert_eq!(tree.selected, vec![].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.selected.insert(ids[3]);
        tree.selected.insert(ids[7]);
        tree.reveal_selection();

        assert_eq!(tree.selected, vec![ids[3], ids[7]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[2], ids[4], ids[6]].into_iter().collect());

        tree.collapse_leaves(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[2], ids[6]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[4]].into_iter().collect());

        tree.collapse_leaves(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[2], ids[4]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.collapse_leaves(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![].into_iter().collect());

        tree.expand_incremental(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.expand_incremental(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[2], ids[4]].into_iter().collect());

        tree.expand_incremental(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[2], ids[4], ids[6]].into_iter().collect());
    }

    #[test]
    fn next() {
        /*
         * 0
         * ├── 1
         * │   ├── 2
         * │   └── 3
         * └── 4
         */
        let ids =
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let files = vec![
            file(0, 0, FileType::Folder, &ids),
            file(1, 0, FileType::Folder, &ids),
            file(2, 1, FileType::Document, &ids),
            file(3, 1, FileType::Document, &ids),
            file(4, 0, FileType::Document, &ids),
        ];

        let mut tree = FileTree::new(files);

        assert_eq!(tree.next(ids[0], false), Some(ids[1]));
        assert_eq!(tree.next(ids[1], false), Some(ids[2]));
        assert_eq!(tree.next(ids[2], false), Some(ids[3]));
        assert_eq!(tree.next(ids[3], false), Some(ids[4]));
        assert_eq!(tree.next(ids[4], false), None);

        assert_eq!(tree.next(ids[0], true), Some(ids[1]));
        assert_eq!(tree.next(ids[1], true), Some(ids[4]));
        assert_eq!(tree.next(ids[2], true), Some(ids[4]));
        assert_eq!(tree.next(ids[3], true), Some(ids[4]));
        assert_eq!(tree.next(ids[4], true), None);

        tree.expand(&[ids[1]]);

        assert_eq!(tree.next(ids[0], false), Some(ids[1]));
        assert_eq!(tree.next(ids[1], false), Some(ids[2]));
        assert_eq!(tree.next(ids[2], false), Some(ids[3]));
        assert_eq!(tree.next(ids[3], false), Some(ids[4]));
        assert_eq!(tree.next(ids[4], false), None);

        assert_eq!(tree.next(ids[0], true), Some(ids[1]));
        assert_eq!(tree.next(ids[1], true), Some(ids[2]));
        assert_eq!(tree.next(ids[2], true), Some(ids[3]));
        assert_eq!(tree.next(ids[3], true), Some(ids[4]));
        assert_eq!(tree.next(ids[4], true), None);

        tree.collapse(&[ids[0]]);

        assert_eq!(tree.next(ids[0], false), Some(ids[1]));
        assert_eq!(tree.next(ids[1], false), Some(ids[2]));
        assert_eq!(tree.next(ids[2], false), Some(ids[3]));
        assert_eq!(tree.next(ids[3], false), Some(ids[4]));
        assert_eq!(tree.next(ids[4], false), None);

        assert_eq!(tree.next(ids[0], true), None);
        assert_eq!(tree.next(ids[1], true), None);
        assert_eq!(tree.next(ids[2], true), None);
        assert_eq!(tree.next(ids[3], true), None);
        assert_eq!(tree.next(ids[4], true), None);
    }

    #[test]
    fn prev() {
        /*
         * 0
         * ├── 1
         * │   ├── 2
         * │   └── 3
         * └── 4
         */
        let ids =
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let files = vec![
            file(0, 0, FileType::Folder, &ids),
            file(1, 0, FileType::Folder, &ids),
            file(2, 1, FileType::Document, &ids),
            file(3, 1, FileType::Document, &ids),
            file(4, 0, FileType::Document, &ids),
        ];

        let mut tree = FileTree::new(files);

        assert_eq!(tree.prev(ids[0], false), None);
        assert_eq!(tree.prev(ids[1], false), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false), Some(ids[3]));

        assert_eq!(tree.prev(ids[0], true), None);
        assert_eq!(tree.prev(ids[1], true), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], true), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], true), Some(ids[1]));
        assert_eq!(tree.prev(ids[4], true), Some(ids[1]));

        tree.expand(&[ids[1]]);

        assert_eq!(tree.prev(ids[0], false), None);
        assert_eq!(tree.prev(ids[1], false), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false), Some(ids[3]));

        assert_eq!(tree.prev(ids[0], true), None);
        assert_eq!(tree.prev(ids[1], false), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false), Some(ids[3]));

        tree.collapse(&[ids[0]]);

        assert_eq!(tree.prev(ids[0], false), None);
        assert_eq!(tree.prev(ids[1], false), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false), Some(ids[3]));

        assert_eq!(tree.prev(ids[0], true), None);
        assert_eq!(tree.prev(ids[1], true), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], true), Some(ids[0]));
        assert_eq!(tree.prev(ids[3], true), Some(ids[0]));
        assert_eq!(tree.prev(ids[4], true), Some(ids[0]));
    }

    #[test]
    fn is_visible() {
        /*
         * 0
         * ├── 1
         * │   ├── 2
         * │   └── 3
         * └── 4
         */
        let ids =
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let files = vec![
            file(0, 0, FileType::Folder, &ids),
            file(1, 0, FileType::Folder, &ids),
            file(2, 1, FileType::Document, &ids),
            file(3, 1, FileType::Document, &ids),
            file(4, 0, FileType::Document, &ids),
        ];

        let mut tree = FileTree::new(files);

        assert!(tree.is_visible(ids[0]));
        assert!(tree.is_visible(ids[1]));
        assert!(!tree.is_visible(ids[2]));
        assert!(!tree.is_visible(ids[3]));
        assert!(tree.is_visible(ids[4]));

        tree.expand(&[ids[1]]);

        assert!(tree.is_visible(ids[0]));
        assert!(tree.is_visible(ids[1]));
        assert!(tree.is_visible(ids[2]));
        assert!(tree.is_visible(ids[3]));
        assert!(tree.is_visible(ids[4]));

        tree.collapse(&[ids[0]]);

        assert!(tree.is_visible(ids[0]));
        assert!(!tree.is_visible(ids[1]));
        assert!(!tree.is_visible(ids[2]));
        assert!(!tree.is_visible(ids[3]));
        assert!(!tree.is_visible(ids[4]));
    }

    fn file(idx: usize, parent_idx: usize, file_type: FileType, ids: &[Uuid]) -> File {
        File {
            id: ids[idx],
            parent: ids[parent_idx],
            name: format!("{idx}"),
            file_type,
            last_modified: Default::default(),
            last_modified_by: Default::default(),
            shares: Default::default(),
        }
    }
}
