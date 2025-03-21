use lb_rs::model::core_config::Config;
use lb_rs::Uuid;
use lb_rs::{blocking::Lb, model::file::File};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Data {
    pub current_root: Uuid,
    pub all_files: HashMap<Uuid, FileRow>,
    pub folder_sizes: HashMap<Uuid, u64>,
    pub overall_root: Uuid,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    pub portion: f32,
    pub children: Vec<Node>,
}

#[derive(PartialEq, Debug, Clone)]
pub struct NodeLayer {
    pub id: Uuid,
    pub name: String,
    pub portion: f32,
    pub layer: u64,
}

#[derive(Debug, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct FileRow {
    pub file: File,
    pub size: u64,
}

impl Data {
    pub fn get_filerows(core: Lb) -> Vec<FileRow> {
        let mut filerows = vec![];
        let usage = core.get_usage().unwrap().usages;
        let meta_data = core.list_metadatas().unwrap();

        for file in meta_data {
            filerows.push(FileRow {
                file: file.clone(),
                size: usage
                    .iter()
                    .find(|item| item.file_id == file.id)
                    .unwrap()
                    .size_bytes,
            });
        }

        return filerows;
    }

    pub fn init(core: Lb) -> Self {
        let data = Self::get_filerows(core);
        let mut all_files = HashMap::new();
        let mut root = Uuid::nil();
        for datum in data.clone() {
            if datum.file.id == datum.file.parent {
                root = datum.file.id;
            }
            all_files.insert(datum.file.id, datum);
        }

        let mut folder_sizes = HashMap::new();
        //Initial for loop for folders is necessary to give folders starting value as we need to go over folders again to update sizes
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

        Self { current_root: root, overall_root: root, all_files, folder_sizes }
    }

    pub fn get_children(&self, id: &Uuid) -> Vec<Node> {
        if !self.all_files.get(id).unwrap().file.is_folder() {
            return vec![];
        }
        let total_size = *self.folder_sizes.get(&self.current_root).unwrap() as f32;
        let children = self
            .all_files
            .values()
            .filter(|f| f.file.parent == *id && f.file.id != *id)
            .map(|f| {
                let mut current_size = f.size as f32;
                if f.file.is_folder() {
                    current_size = *self.folder_sizes.get(&f.file.id).unwrap() as f32;
                }
                Node {
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
        gathered_children.sort_by(|a, b| {
            let a_size = (a.portion * 10000.0) as u32;
            let b_size = (b.portion * 10000.0) as u32;
            b_size.cmp(&a_size)
        });
        return gathered_children;
    }

    fn set_layers(
        tree: &Vec<Node>, current_layer: u64, mut raw_layers: Vec<NodeLayer>,
    ) -> Vec<NodeLayer> {
        for slice in tree {
            raw_layers.push(NodeLayer {
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
        return raw_layers;
    }

    pub fn get_paint_order(&self) -> Vec<NodeLayer> {
        let tree = self.get_children(&self.current_root);
        let mut paint_order_vec = Data::set_layers(&tree, 1, vec![]);
        paint_order_vec.sort_by(|a, b| a.layer.cmp(&b.layer));
        return paint_order_vec;
    }
}
