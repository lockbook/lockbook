use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};

use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::service::usage::UsageMetrics;
use lb_rs::Uuid;

pub struct FileCache {
    pub files: Vec<File>,
    pub suggested: Vec<Uuid>,
    pub usage: UsageMetrics,
}

impl FileCache {
    pub fn new(lb: &Lb) -> LbResult<Self> {
        Ok(Self {
            files: lb.list_metadatas()?,
            suggested: lb
                .suggested_docs(Default::default())?
                .into_iter()
                .take(5)
                .collect(),
            usage: lb.get_usage()?,
        })
    }
}

impl Debug for FileCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCache")
            .field("files", &self.files.len())
            .field("suggested", &self.suggested.len())
            .field("usage", &self.usage.usages.len())
            .finish()
    }
}

pub trait FilesExt {
    fn root(&self) -> Uuid;
    fn get_by_id(&self, id: Uuid) -> Option<&File>;
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

    fn descendents(&self, id: Uuid) -> Vec<&File> {
        let mut descendents = vec![];
        for child in self.children(id) {
            descendents.extend(self.descendents(child.id));
            descendents.push(child);
        }
        descendents
    }
}

impl FilesExt for Vec<File> {
    fn root(&self) -> Uuid {
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
