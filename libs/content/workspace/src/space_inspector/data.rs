use crate::file_cache::FilesExt as _;
use lb_rs::Uuid;
use lb_rs::model::api::FileUsage;
use lb_rs::model::file::File;
use serde::Deserialize;
use std::collections::HashMap;

/// Contains data related to folders and files needed for space inspector
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Data {
    pub focused_folder: Uuid,
    pub all_files: HashMap<Uuid, FileRow>,
    pub folder_sizes: HashMap<Uuid, u64>,
    pub root: Uuid,
}

#[derive(Debug, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct FileRow {
    pub file: File,
    pub size: u64,
}

/// Stores information in a tree format returned by get_children()
#[derive(PartialEq, Debug, Clone)]
pub struct StorageTree {
    pub id: Uuid,
    name: String,
    portion: f32,
    pub children: Vec<StorageTree>,
}

/// Responsible for storing relevant folder information for painting. Portion represents the ratio of size between the file and the root
#[derive(PartialEq, Debug, Clone)]
pub struct StorageCell {
    pub id: Uuid,
    pub name: String,
    pub portion: f32,
    pub layer: u64,
}

impl Data {
    pub fn init(potential_root: Option<File>, usage: Vec<FileUsage>, meta_data: Vec<File>) -> Self {
        let root = meta_data.root();
        let data = Self::get_filerows(usage, meta_data);
        let mut all_files = HashMap::new();
        for datum in data.clone() {
            all_files.insert(datum.file.id, datum);
        }

        let mut folder_sizes = HashMap::new();
        // Initialize folders with default size, then each document's size is added to each of that document's ancestors
        for datum in data.clone() {
            if datum.file.is_folder() {
                folder_sizes.insert(datum.file.id, datum.size);
            }
        }
        for datum in data {
            let datum_size = datum.size;
            let mut current_id = datum.file.id;
            loop {
                let row = &all_files[&current_id];
                let mut current_size = folder_sizes
                    .get(&row.file.parent)
                    .copied()
                    .unwrap_or_default();
                current_size += datum_size;
                if current_id == root {
                    break;
                }
                folder_sizes.insert(row.file.parent, current_size);
                current_id = row.file.parent;
            }
        }

        let focused_folder = match potential_root {
            Some(focused_folder) => focused_folder.id,
            None => root,
        };

        Self { focused_folder, root, all_files, folder_sizes }
    }

    fn get_filerows(usage: Vec<FileUsage>, meta_data: Vec<File>) -> Vec<FileRow> {
        let mut filerows = vec![];

        for file in meta_data {
            filerows.push(FileRow {
                size: usage
                    .iter()
                    .find(|item| item.file_id == file.id)
                    .unwrap_or(&FileUsage { file_id: file.id, size_bytes: 0 }) // Files that are shared with you take up 0. In the future, we may implement a way to view it with the flag that its not stored by you
                    .size_bytes,
                file,
            });
        }

        filerows
    }

    pub fn get_size(&self, id: &Uuid) -> u64 {
        if self.all_files[id].file.is_folder() {
            self.folder_sizes[id]
        } else {
            self.all_files[id].size
        }
    }

    pub fn is_folder(&self, id: &Uuid) -> bool {
        self.all_files[id].file.is_folder()
    }

    pub fn get_children(&self, id: &Uuid) -> Vec<StorageTree> {
        if !self.all_files[id].file.is_folder() {
            return vec![];
        }
        let total_size = self.folder_sizes[&self.focused_folder] as f32;
        let children = self
            .all_files
            .values()
            .filter(|f| f.file.parent == *id && !f.file.is_root())
            .map(|f| {
                let mut current_size = f.size as f32;
                if f.file.is_folder() {
                    current_size = self.folder_sizes[&f.file.id] as f32;
                }
                StorageTree {
                    id: f.file.id,
                    name: f.file.name.clone(),
                    portion: current_size / total_size,
                    children: self.get_children(&f.file.id),
                }
            });
        let mut gathered_children: Vec<_> = children.collect();
        gathered_children.sort_by(|a, b| b.portion.partial_cmp(&a.portion).unwrap());
        gathered_children
    }

    /// Recursive function that goes through StorageTrees extracted StorageCells from each layer
    fn set_layers(
        trees: &Vec<StorageTree>, layer: u64, mut cells: Vec<StorageCell>,
    ) -> Vec<StorageCell> {
        for tree in trees {
            cells.push(StorageCell {
                id: tree.id,
                name: tree.name.clone(),
                portion: tree.portion,
                layer,
            });
            if !tree.children.is_empty() {
                let next_layer_cells = Data::set_layers(&tree.children, layer + 1, cells.clone());
                for cell in next_layer_cells {
                    if cells.contains(&cell) {
                        continue;
                    }
                    cells.push(cell.clone());
                }
            }
        }
        cells
    }

    /// Gets the order of StorageCells and sorts by layers with higher size .
    /// Example:
    /// [Layer 1: 10 KB, Layer 1: 3 KB, Layer 2: 2 KB]
    pub fn get_paint_order(&self) -> Vec<StorageCell> {
        let trees = self.get_children(&self.focused_folder); // gets all children of the root in StorageTree format
        let mut paint_order_vec = Data::set_layers(&trees, 1, vec![]);
        paint_order_vec.sort_by(|a, b| a.layer.cmp(&b.layer));
        paint_order_vec
    }
}
