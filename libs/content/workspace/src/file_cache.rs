use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;

pub struct FileCache {
    pub files: Vec<File>,
    pub shared: Vec<File>,
    pub suggested: Vec<Uuid>,
}

impl FileCache {
    pub fn new(lb: &Lb) -> LbResult<Self> {
        Ok(Self {
            files: lb.list_metadatas()?,
            suggested: lb.suggested_docs(Default::default())?,
            shared: lb.get_pending_share_files()?,
        })
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
    fn descendents(&self, id: Uuid) -> Vec<&File>;

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
