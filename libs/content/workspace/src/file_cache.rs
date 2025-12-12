use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::iter;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;

pub struct FileCache {
    pub files: Vec<File>,
    pub shared: Vec<File>,
    pub suggested: Vec<Uuid>,
    pub usage: HashMap<Uuid, usize>,
}

impl FileCache {
    pub fn new(lb: &Lb) -> LbResult<Self> {
        Ok(Self {
            files: lb.list_metadatas()?,
            suggested: lb.suggested_docs(Default::default())?,
            shared: lb.get_pending_share_files()?,
            usage: lb.get_uncompressed_usage_breakdown()?,
        })
    }

    pub fn uncompressed_usage_recursive(&self, id: Uuid) -> usize {
        self.files
            .descendents(id)
            .iter()
            .map(|f| f.id)
            .chain(iter::once(id))
            .filter_map(|id| self.usage.get(&id))
            .sum::<_>()
    }

    pub fn usage_portion(&self, id: Uuid) -> f32 {
        self.uncompressed_usage_recursive(id) as f32
            / self.uncompressed_usage_recursive(self.files.get_by_id(id).unwrap().parent) as f32
    }

    /// returns the uncompressed, recursive size of a file scaled relative to
    /// siblings so that the biggest sibling is 1.0
    pub fn usage_portion_scaled(&self, id: Uuid) -> f32 {
        let siblings = self.files.siblings(id);
        let current_usage = self.uncompressed_usage_recursive(id);

        let max_sibling_usage = siblings
            .iter()
            .map(|sibling| self.uncompressed_usage_recursive(sibling.id))
            .chain(std::iter::once(current_usage))
            .max()
            .unwrap_or(1);

        current_usage as f32 / max_sibling_usage as f32
    }

    pub fn last_modified_recursive(&self, id: Uuid) -> u64 {
        self.files
            .descendents(id)
            .iter()
            .map(|f| f.id)
            .chain(iter::once(id))
            .map(|id| self.files.get_by_id(id).unwrap().last_modified)
            .max()
            .unwrap()
    }

    pub fn last_modified_by_recursive(&self, id: Uuid) -> &str {
        let last_modified_id = self
            .files
            .descendents(id)
            .iter()
            .map(|f| f.id)
            .chain(iter::once(id))
            .max_by_key(|id| self.files.get_by_id(*id).unwrap().last_modified)
            .unwrap();
        &self
            .files
            .get_by_id(last_modified_id)
            .unwrap()
            .last_modified_by
    }
}

impl Debug for FileCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCache")
            .field("files.len()", &self.files.len())
            .field("suggested.len()", &self.suggested.len())
            .finish()
    }
}

pub trait FilesExt {
    fn root(&self) -> &File;
    fn get_by_id(&self, id: Uuid) -> Option<&File>;
    fn children(&self, id: Uuid) -> Vec<&File>;

    fn siblings(&self, id: Uuid) -> Vec<&File> {
        let parent = self.get_by_id(id).unwrap().parent;
        self.children(parent)
            .into_iter()
            .filter(|f| f.id != id)
            .collect()
    }

    fn descendents(&self, id: Uuid) -> Vec<&File> {
        let mut descendents = vec![];
        for child in self.children(id) {
            descendents.extend(self.descendents(child.id));
            descendents.push(child);
        }
        descendents
    }

    /// returns all known parents until we can't find one (share) or we hit root
    fn ancestors(&self, id: Uuid) -> Vec<Uuid> {
        let mut ancestors = vec![];
        if let Some(us) = self.get_by_id(id) {
            if us.is_root() {
                return ancestors;
            }

            let parent = us.parent;
            ancestors.push(parent);
            ancestors.extend_from_slice(&self.ancestors(parent));
        }
        ancestors
    }
}

impl FilesExt for [File] {
    fn root(&self) -> &File {
        for file in self {
            if file.is_root() {
                return file;
            }
        }
        unreachable!("unable to find root in metadata list")
    }

    fn get_by_id(&self, id: Uuid) -> Option<&File> {
        self.iter().find(|f| f.id == id)
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

impl FilesExt for Vec<File> {
    fn root(&self) -> &File {
        self.as_slice().root()
    }

    fn get_by_id(&self, id: Uuid) -> Option<&File> {
        self.as_slice().get_by_id(id)
    }

    fn children(&self, id: Uuid) -> Vec<&File> {
        self.as_slice().children(id)
    }

    fn descendents(&self, id: Uuid) -> Vec<&File> {
        self.as_slice().descendents(id)
    }
}
