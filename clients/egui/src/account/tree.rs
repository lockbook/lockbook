use std::{cmp::Ordering, collections::HashSet, mem};

use lb::{
    model::{file::File, file_metadata::FileType},
    Uuid,
};

#[derive(Debug)]
pub struct FileTree {
    /// Set of selected files. To be selected, a file must be visible i.e. all its ancestors are expanded.
    pub selected: HashSet<Uuid>,

    /// Set of expanded files. To be expanded, a file must be a folder but need not be visible.
    pub expanded: HashSet<Uuid>,
}

impl FileTree {
    pub fn new(files: &[File]) -> Self {
        Self { selected: HashSet::new(), expanded: [files.root()].into_iter().collect() }
    }

    /// Clears the selection. Does not expand or collapse anything.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Adds `ids` to the selection (optionally `recursive`) and reveals them in the tree.
    pub fn select(&mut self, ids: &[Uuid], recursive: bool, files: &[File]) {
        self.selected.extend(ids);
        if recursive {
            for &id in ids {
                self.selected
                    .extend(files.descendents(id).into_iter().map(|f| f.id));
            }
        }
        self.reveal_selection(files);
    }

    /// Removes `ids` from the selection (optionally `recursive`). Does not expand or collapse anything.
    pub fn deselect(&mut self, ids: &[Uuid], recursive: bool, files: &[File]) {
        for &id in ids {
            self.selected.remove(&id);
            if recursive {
                for descendent in files.descendents(id) {
                    self.selected.remove(&descendent.id);
                }
            }
        }
    }

    /// Expands `ids`. Does not select or deselect anything.
    pub fn expand(&mut self, ids: &[Uuid], recursive: bool, files: &[File]) {
        self.expanded.extend(ids);
        if recursive {
            for &id in ids {
                self.expanded
                    .extend(files.descendents(id).into_iter().filter_map(|f| {
                        if let FileType::Folder = f.file_type {
                            Some(f.id)
                        } else {
                            None
                        }
                    }));
            }
        }
    }

    /// Collapses `ids`. Selections that are hidden are replaced with their closest visible ancestor.
    pub fn collapse(&mut self, ids: &[Uuid], recursive: bool, files: &[File]) {
        for &id in ids {
            self.expanded.remove(&id);
            if recursive {
                for descendent in files.descendents(id) {
                    self.expanded.remove(&descendent.id);
                }
            }
        }
        self.select_visible_ancestors(files);
    }

    /// A file is visible if all its ancestors are expanded.
    fn is_visible(&self, id: Uuid, files: &[File]) -> bool {
        let file = files.get_by_id(id);
        if file.parent == file.id {
            return true;
        }
        self.expanded.contains(&file.parent) && self.is_visible(file.parent, files)
    }

    /// Replaces each file in selection with its first visible ancestor (including itself).
    fn select_visible_ancestors(&mut self, files: &[File]) {
        let selected = mem::take(&mut self.selected);
        for mut id in selected {
            while !self.is_visible(id, files) {
                id = files.get_by_id(id).parent;
            }
            self.selected.insert(id);
        }
    }

    /// Expands the parents of the selected files.
    fn reveal_selection(&mut self, files: &[File]) {
        for mut id in self.selected.clone() {
            loop {
                let parent = files.get_by_id(id).parent;

                if parent == id {
                    break;
                }

                self.expanded.insert(parent);
                id = parent;
            }
        }
    }

    /// Returns the file after id in depth-first order, folders first then alphabetically.
    fn next(&self, id: Uuid, visible_only: bool, files: &[File]) -> Option<Uuid> {
        // if the file has children, return the first child
        // if `visible_only` is true then the child must be visible i.e. the file must be visible and expanded
        if !visible_only || (self.is_visible(id, files) && self.expanded.contains(&id)) {
            if let Some(first_child) = files.children(id).first() {
                return Some(first_child.id);
            }
        }

        // otherwise, return the next sibling of the file's closest ancestor (including itself) that has a next sibling
        let mut ancestor = id;
        loop {
            let parent = files.get_by_id(ancestor).parent;
            if !visible_only || self.is_visible(ancestor, files) {
                let siblings = files.children(parent);
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
    fn prev(&self, id: Uuid, visible_only: bool, files: &[File]) -> Option<Uuid> {
        let parent = files.get_by_id(id).parent;
        if id == parent {
            return None;
        }

        let siblings = files.children(parent);
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
                let children = files.children(last_descendent);
                if let Some(last_child) = children.last() {
                    last_descendent = last_child.id;
                } else {
                    break;
                }
            }
            if visible_only {
                loop {
                    if self.is_visible(last_descendent, files) {
                        break;
                    }
                    last_descendent = files.get_by_id(last_descendent).parent;
                }
            }
            Some(last_descendent)
        } else {
            // if the file is the first child of its parent, return the parent
            // if `visible_only` is true then return the first visible ancestor of the parent (including the parent)
            let mut ancestor = files.get_by_id(id).parent;
            if visible_only {
                loop {
                    if self.is_visible(ancestor, files) {
                        break;
                    }
                    ancestor = files.get_by_id(ancestor).parent;
                }
            }
            Some(ancestor)
        }
    }
}

trait FilesExt {
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

        let mut tree = FileTree::new(&files);

        assert_eq!(tree.selected, vec![].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.select(&[ids[1]], false, &files);

        assert_eq!(tree.selected, vec![ids[1]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.select(&[ids[1]], true, &files);

        assert_eq!(tree.selected, vec![ids[1], ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.deselect(&[ids[1]], false, &files);

        assert_eq!(tree.selected, vec![ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.deselect(&[ids[1]], true, &files);

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

        let mut tree = FileTree::new(&files);

        tree.collapse(&[ids[0]], false, &files);
        tree.select(&[ids[0]], false, &files);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![].into_iter().collect());

        tree.expand(&[ids[0]], false, &files);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0]].into_iter().collect());

        tree.expand(&[ids[0]], true, &files);
        tree.clear_selection();
        tree.select(&[ids[2], ids[3]], false, &files);

        assert_eq!(tree.selected, vec![ids[2], ids[3]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[0], ids[1]].into_iter().collect());

        tree.collapse(&[ids[0]], false, &files);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![ids[1]].into_iter().collect());

        tree.expand(&[ids[0]], true, &files);
        tree.clear_selection();
        tree.select(&[ids[2], ids[3]], false, &files);
        tree.collapse(&[ids[0]], true, &files);

        assert_eq!(tree.selected, vec![ids[0]].into_iter().collect());
        assert_eq!(tree.expanded, vec![].into_iter().collect());
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

        let mut tree = FileTree::new(&files);

        assert!(tree.is_visible(ids[0], &files));
        assert!(tree.is_visible(ids[1], &files));
        assert!(!tree.is_visible(ids[2], &files));
        assert!(!tree.is_visible(ids[3], &files));
        assert!(tree.is_visible(ids[4], &files));

        tree.expand(&[ids[1]], false, &files);

        assert!(tree.is_visible(ids[0], &files));
        assert!(tree.is_visible(ids[1], &files));
        assert!(tree.is_visible(ids[2], &files));
        assert!(tree.is_visible(ids[3], &files));
        assert!(tree.is_visible(ids[4], &files));

        tree.collapse(&[ids[0]], false, &files);

        assert!(tree.is_visible(ids[0], &files));
        assert!(!tree.is_visible(ids[1], &files));
        assert!(!tree.is_visible(ids[2], &files));
        assert!(!tree.is_visible(ids[3], &files));
        assert!(!tree.is_visible(ids[4], &files));
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

        let mut tree = FileTree::new(&files);

        assert_eq!(tree.next(ids[0], false, &files), Some(ids[1]));
        assert_eq!(tree.next(ids[1], false, &files), Some(ids[2]));
        assert_eq!(tree.next(ids[2], false, &files), Some(ids[3]));
        assert_eq!(tree.next(ids[3], false, &files), Some(ids[4]));
        assert_eq!(tree.next(ids[4], false, &files), None);

        assert_eq!(tree.next(ids[0], true, &files), Some(ids[1]));
        assert_eq!(tree.next(ids[1], true, &files), Some(ids[4]));
        assert_eq!(tree.next(ids[2], true, &files), Some(ids[4]));
        assert_eq!(tree.next(ids[3], true, &files), Some(ids[4]));
        assert_eq!(tree.next(ids[4], true, &files), None);

        tree.expand(&[ids[1]], false, &files);

        assert_eq!(tree.next(ids[0], false, &files), Some(ids[1]));
        assert_eq!(tree.next(ids[1], false, &files), Some(ids[2]));
        assert_eq!(tree.next(ids[2], false, &files), Some(ids[3]));
        assert_eq!(tree.next(ids[3], false, &files), Some(ids[4]));
        assert_eq!(tree.next(ids[4], false, &files), None);

        assert_eq!(tree.next(ids[0], true, &files), Some(ids[1]));
        assert_eq!(tree.next(ids[1], true, &files), Some(ids[2]));
        assert_eq!(tree.next(ids[2], true, &files), Some(ids[3]));
        assert_eq!(tree.next(ids[3], true, &files), Some(ids[4]));
        assert_eq!(tree.next(ids[4], true, &files), None);

        tree.collapse(&[ids[0]], false, &files);

        assert_eq!(tree.next(ids[0], false, &files), Some(ids[1]));
        assert_eq!(tree.next(ids[1], false, &files), Some(ids[2]));
        assert_eq!(tree.next(ids[2], false, &files), Some(ids[3]));
        assert_eq!(tree.next(ids[3], false, &files), Some(ids[4]));
        assert_eq!(tree.next(ids[4], false, &files), None);

        assert_eq!(tree.next(ids[0], true, &files), None);
        assert_eq!(tree.next(ids[1], true, &files), None);
        assert_eq!(tree.next(ids[2], true, &files), None);
        assert_eq!(tree.next(ids[3], true, &files), None);
        assert_eq!(tree.next(ids[4], true, &files), None);
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

        let mut tree = FileTree::new(&files);

        assert_eq!(tree.prev(ids[0], false, &files), None);
        assert_eq!(tree.prev(ids[1], false, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false, &files), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false, &files), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false, &files), Some(ids[3]));

        assert_eq!(tree.prev(ids[0], true, &files), None);
        assert_eq!(tree.prev(ids[1], true, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], true, &files), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], true, &files), Some(ids[1]));
        assert_eq!(tree.prev(ids[4], true, &files), Some(ids[1]));

        tree.expand(&[ids[1]], false, &files);

        assert_eq!(tree.prev(ids[0], false, &files), None);
        assert_eq!(tree.prev(ids[1], false, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false, &files), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false, &files), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false, &files), Some(ids[3]));

        assert_eq!(tree.prev(ids[0], true, &files), None);
        assert_eq!(tree.prev(ids[1], false, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false, &files), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false, &files), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false, &files), Some(ids[3]));

        tree.collapse(&[ids[0]], false, &files);

        assert_eq!(tree.prev(ids[0], false, &files), None);
        assert_eq!(tree.prev(ids[1], false, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], false, &files), Some(ids[1]));
        assert_eq!(tree.prev(ids[3], false, &files), Some(ids[2]));
        assert_eq!(tree.prev(ids[4], false, &files), Some(ids[3]));

        assert_eq!(tree.prev(ids[0], true, &files), None);
        assert_eq!(tree.prev(ids[1], true, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[2], true, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[3], true, &files), Some(ids[0]));
        assert_eq!(tree.prev(ids[4], true, &files), Some(ids[0]));
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
