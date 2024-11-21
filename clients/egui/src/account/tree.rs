use std::{cmp::Ordering, collections::HashSet, mem, path::PathBuf};

use egui::{Id, Ui};
use lb::{
    logic::filename::DocumentType,
    model::{file::File, file_metadata::FileType},
    Uuid,
};
use workspace_rs::{theme::icons::Icon, widgets::Button};

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
}

impl FileTree {
    pub fn new(files: Vec<File>) -> Self {
        Self {
            selected: HashSet::new(),
            expanded: [files.root()].into_iter().collect(),
            files,
            cursor: None,
        }
    }

    /// Updates the files in the tree. The selection and expansion are preserved except that the selection is revealed
    /// in the new tree.
    pub fn update_files(&mut self, files: Vec<File>) {
        self.files = files;
        self.expanded
            .retain(|&id| self.files.iter().any(|f| f.id == id));
        self.selected
            .retain(|&id| self.files.iter().any(|f| f.id == id));
        self.reveal_selection();
    }

    /// Adds `ids` to the selection and reveals them in the tree.
    pub fn select(&mut self, ids: &[Uuid]) {
        self.selected.extend(ids);
        self.reveal_selection();
    }

    /// Adds `ids` to the selection and reveals them recursively to a maximum depth of `depth`. If `depth` is `None`,
    /// selects all the way.
    pub fn select_recursive(&mut self, ids: &[Uuid], depth: Option<usize>) {
        self.selected.extend(ids.iter().copied());
        if depth == Some(0) {
            return;
        }
        for &id in ids {
            let children = self
                .files
                .children(id)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            for child in children {
                self.select_recursive(&[child.id], depth.map(|d| d - 1));
            }
        }
        self.reveal_selection();
    }

    /// Removes `ids` from the selection. Does not expand or collapse anything.
    pub fn deselect(&mut self, ids: &[Uuid]) {
        self.selected.retain(|&id| !ids.contains(&id));
    }

    /// Clears the selection. Does not expand or collapse anything.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Expands `ids`. Does not select or deselect anything.
    pub fn expand(&mut self, ids: &[Uuid]) {
        self.expand_recursive(ids, Some(0));
    }

    /// Expands `ids` recursively to a maximum depth of `depth`. If `depth` is `None`, expands all the way. Does not
    /// select or deselect anything.
    pub fn expand_recursive(&mut self, ids: &[Uuid], depth: Option<usize>) {
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
    pub fn expand_incremental(&mut self, ids: &[Uuid]) {
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
    pub fn collapse(&mut self, ids: &[Uuid]) {
        self.expanded.retain(|&id| !ids.contains(&id));
        self.select_visible_ancestors();
    }

    /// Collapses all leaves under `ids`. Selections that are hidden are replaced with their closest visible ancestor.
    pub fn collapse_leaves(&mut self, ids: &[Uuid]) {
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
    fn reveal_selection(&mut self) {
        for mut id in self.selected.clone() {
            loop {
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
    pub fn next(&self, id: Uuid, visible_only: bool) -> Option<Uuid> {
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
    pub fn prev(&self, id: Uuid, visible_only: bool) -> Option<Uuid> {
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

    /// A file is visible if all its ancestors are expanded.
    pub fn is_visible(&self, id: Uuid) -> bool {
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
    pub rename_request: Option<(Uuid, String)>,
    pub delete_request: bool,
    pub dropped_on: Option<Uuid>,
}

impl Response {
    pub fn union(self, other: Self) -> Self {
        let mut this = self;
        this.new_file = this.new_file.or(other.new_file);
        this.new_drawing = this.new_drawing.or(other.new_drawing);
        this.new_folder_modal = this.new_folder_modal.or(other.new_folder_modal);
        this.create_share_modal = this.create_share_modal.or(other.create_share_modal);
        this.export_file = this.export_file.or(other.export_file);
        this.open_requests.extend(other.open_requests);
        this.rename_request = this.rename_request.or(other.rename_request);
        this.delete_request = this.delete_request || other.delete_request;
        this.dropped_on = this.dropped_on.or(other.dropped_on);
        this
    }
}

impl FileTree {
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        ui.vertical(|ui| self.show_recursive(ui, self.files.root()))
            .inner
    }

    pub fn show_recursive(&mut self, ui: &mut Ui, id: Uuid) -> Response {
        let mut resp = Response::default();

        let file = self.files.get_by_id(id);
        // let is_selected = self.selected.contains(&id);
        // let is_visible = self.is_visible(id);

        if file.is_document() {
            let doc_type = DocumentType::from_file_name_using_extension(&file.name);
            let button_resp = match doc_type {
                DocumentType::Text => Button::default()
                    .icon(&Icon::DOC_TEXT)
                    .text(&file.name)
                    .show(ui),
                DocumentType::Drawing => Button::default()
                    .icon(&Icon::DRAW)
                    .text(&file.name)
                    .show(ui),
                DocumentType::Other => Button::default()
                    .icon(&Icon::DOC_UNKNOWN)
                    .text(&file.name)
                    .show(ui),
            };

            if button_resp.clicked() {
                resp.open_requests.insert(id);
            }
        } else {
            let is_expanded = self.expanded.contains(&id);
            let is_shared = !file.shares.is_empty();
            let button_resp = if is_expanded {
                let button_resp = Button::default()
                    .icon(&Icon::FOLDER_OPEN)
                    .text(&file.name)
                    .show(ui);
                resp = resp.union(
                    ui.indent(Id::new(id.to_string()), |ui| self.show_children_recursive(ui, id))
                        .inner,
                );
                button_resp
            } else if is_shared {
                Button::default()
                    .icon(&Icon::SHARED_FOLDER)
                    .text(&file.name)
                    .show(ui)
            } else {
                Button::default()
                    .icon(&Icon::FOLDER)
                    .text(&file.name)
                    .show(ui)
            };

            if button_resp.clicked() {
                if !is_expanded {
                    self.expand(&[id]);
                } else {
                    self.collapse(&[id]);
                }
            }
        };

        resp
    }

    pub fn show_children_recursive(&mut self, ui: &mut Ui, id: Uuid) -> Response {
        let children_ids = self
            .files
            .children(id)
            .iter()
            .map(|f| f.id)
            .collect::<Vec<_>>();
        let mut resp = Response::default();
        for child in children_ids {
            resp = resp.union(self.show_recursive(ui, child));
        }
        resp
    }
}

pub trait FilesExt {
    fn root(&self) -> Uuid;
    fn get_by_id(&self, id: Uuid) -> &File;
    fn children(&self, id: Uuid) -> Vec<&File>;
    fn descendents(&self, id: Uuid) -> Vec<&File>;
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

    fn descendents(&self, id: Uuid) -> Vec<&File> {
        let mut descendents = vec![];
        for child in self.children(id) {
            descendents.extend(self.descendents(child.id));
            descendents.push(child);
        }
        descendents
    }
}

#[cfg(test)]
mod test {
    use lb::{
        model::{file::File, file_metadata::FileType},
        Uuid,
    };

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

        tree.select(&[ids[1]]);

        assert_eq!(tree.selected, vec![ids[1]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.select_recursive(&[ids[1]], None);

        assert_eq!(tree.selected, vec![ids[1], ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.deselect(&[ids[1]]);

        assert_eq!(tree.selected, vec![ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.clear_selection();

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
        tree.select(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![].into_iter().collect());

        tree.expand(&[ids[0]]);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.expand_recursive(&[ids[0]], None);
        tree.clear_selection();
        tree.select(&[ids[2], ids[3]]);

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

        tree.select(&[ids[3], ids[7]]);

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
            name: format!("{}", idx),
            file_type,
            last_modified: Default::default(),
            last_modified_by: Default::default(),
            shares: Default::default(),
        }
    }
}
