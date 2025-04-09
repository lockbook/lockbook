use lb_rs::model::api::FileUsage;
use lb_rs::model::file::File;
use lb_rs::Uuid;
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
    id: Uuid,
    name: String,
    portion: f32,
    children: Vec<StorageTree>,
}

/// Responsible for storing relevant folder information for painting
#[derive(PartialEq, Debug, Clone)]
pub struct StorageCell {
    pub id: Uuid,
    pub name: String,
    pub portion: f32,
    pub layer: u64,
}

impl Data {
    pub fn init(potential_root: Option<File>, usage: Vec<FileUsage>, meta_data: Vec<File>) -> Self {
        let data = Self::get_filerows(usage, meta_data);
        let mut all_files = HashMap::new();
        let mut root = Uuid::nil();
        for datum in data.clone() {
            if datum.file.is_root() {
                root = datum.file.id;
            }
            all_files.insert(datum.file.id, datum);
        }

        if root.is_nil() {
            panic!("No root exists");
        }

        let mut folder_sizes = HashMap::new();
        // Initial for loop for folders is necessary to give folders starting value as we need to go over folders again to update sizes
        for datum in data.clone() {
            if datum.file.is_folder() {
                folder_sizes.insert(datum.file.id, datum.size);
            }
        }
        for datum in data {
            let datum_size = datum.size;
            let mut current_id = datum.file.id;
            loop {
                let row = all_files.get(&current_id).unwrap();
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
                    .unwrap_or(&FileUsage { file_id: file.id, size_bytes: 0 })
                    .size_bytes,
                file,
            });
        }

        filerows
    }

    fn get_children(&self, id: &Uuid) -> Vec<StorageTree> {
        if !self.all_files.get(id).unwrap().file.is_folder() {
            return vec![];
        }
        let total_size = *self.folder_sizes.get(&self.focused_folder).unwrap() as f32;
        let children = self
            .all_files
            .values()
            .filter(|f| f.file.parent == *id && f.file.id != *id)
            .map(|f| {
                let mut current_size = f.size as f32;
                if f.file.is_folder() {
                    current_size = *self.folder_sizes.get(&f.file.id).unwrap() as f32;
                }
                StorageTree {
                    id: f.file.id,
                    name: f.file.name.clone(),
                    portion: current_size / total_size,
                    children: self.get_children(&f.file.id),
                }
            });
        let mut gathered_children = vec![];
        for child in children.into_iter() {
            gathered_children.push(child);
        }
        gathered_children.sort_by(|a, b| b.portion.partial_cmp(&a.portion).unwrap());
        gathered_children
    }

    fn set_layers(
        tree: &Vec<StorageTree>, current_layer: u64, mut raw_layers: Vec<StorageCell>,
    ) -> Vec<StorageCell> {
        for slice in tree {
            raw_layers.push(StorageCell {
                id: slice.id,
                name: slice.name.clone(),
                portion: slice.portion,
                layer: current_layer,
            });
            if !slice.children.is_empty() {
                let hold = Data::set_layers(&slice.children, current_layer + 1, raw_layers.clone());
                for item in hold {
                    if raw_layers.contains(&item) {
                        continue;
                    }
                    raw_layers.push(item.clone());
                }
            }
        }
        raw_layers
    }

    pub fn get_paint_order(&self) -> Vec<StorageCell> {
        let tree = self.get_children(&self.focused_folder);
        let mut paint_order_vec = Data::set_layers(&tree, 1, vec![]);
        paint_order_vec.sort_by(|a, b| a.layer.cmp(&b.layer));
        paint_order_vec
    }
}
