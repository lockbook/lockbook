use crate::file_metadata::FileType::{Document, Folder};
use crate::file_metadata::{FileType, Owner};
use crate::tree::TreeError::{FileNonexistent, RootNonexistent};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use uuid::Uuid;

pub trait FileMetadata: Clone + Display {
    type Name: Hash + Eq;

    fn id(&self) -> Uuid;
    fn file_type(&self) -> FileType;
    fn parent(&self) -> Uuid;
    fn name(&self) -> Self::Name;
    fn owner(&self) -> Owner;
    fn metadata_version(&self) -> u64;
    fn content_version(&self) -> u64;
    fn deleted(&self) -> bool;
    fn display(&self) -> String;

    fn is_folder(&self) -> bool {
        self.file_type() == Folder
    }

    fn is_document(&self) -> bool {
        self.file_type() == Document
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeError {
    RootNonexistent,
    FileNonexistent,
    FileParentNonexistent,
    Unexpected(String),
}

#[derive(Clone)]
pub enum StageSource {
    Base,
    Staged,
}

#[derive(Debug, PartialEq)]
pub struct PathConflict {
    pub existing: Uuid,
    pub staged: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TestFileTreeError {
    NoRootFolder,
    DocumentTreatedAsFolder(Uuid),
    FileOrphaned(Uuid),
    CycleDetected(Uuid),
    NameConflictDetected(Uuid),
    Tree(TreeError),
}

pub trait FileMetaVecExt<T: FileMetadata> {
    fn to_map(&self) -> HashMap<Uuid, T>;
}

impl<Fm> FileMetaVecExt<Fm> for [Fm]
where
    Fm: FileMetadata,
{
    fn to_map(&self) -> HashMap<Uuid, Fm> {
        self.iter()
            .map(|f| (f.id(), f.clone()))
            .collect::<HashMap<Uuid, Fm>>()
    }
}

pub trait FileMetaMapExt<Fm: FileMetadata> {
    fn with(fm: Fm) -> HashMap<Uuid, Fm>;
    fn ids(&self) -> Vec<Uuid>;
    fn push(&mut self, fm: Fm);
    fn stage(&self, staged: &HashMap<Uuid, Fm>) -> HashMap<Uuid, (Fm, StageSource)>;
    fn find(&self, id: Uuid) -> Result<Fm, TreeError>;
    fn find_mut(&mut self, id: Uuid) -> Result<&mut Fm, TreeError>;
    fn find_root(&self) -> Result<Fm, TreeError>;
    fn maybe_find_root(&self) -> Option<Fm>;
    fn maybe_find(&self, id: Uuid) -> Option<Fm>;
    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut Fm>;
    fn find_parent(&self, id: Uuid) -> Result<Fm, TreeError>;
    fn maybe_find_parent(&self, id: Uuid) -> Option<Fm>;
    fn find_children(&self, id: Uuid) -> HashMap<Uuid, Fm>;
    fn filter_deleted(&self) -> Result<HashMap<Uuid, Fm>, TreeError>;
    fn filter_not_deleted(&self) -> Result<HashMap<Uuid, Fm>, TreeError>;
    fn filter_documents(&self) -> HashMap<Uuid, Fm>;
    fn get_invalid_cycles(
        &self, staged_changes: &HashMap<Uuid, Fm>,
    ) -> Result<Vec<Uuid>, TreeError>;
    fn get_path_conflicts(
        &self, staged_changes: &HashMap<Uuid, Fm>,
    ) -> Result<Vec<PathConflict>, TreeError>;
    fn verify_integrity(&self) -> Result<(), TestFileTreeError>;
    fn pretty_print(&self) -> String;
}

impl<Fm> FileMetaMapExt<Fm> for HashMap<Uuid, Fm>
where
    Fm: FileMetadata,
{
    fn with(fm: Fm) -> HashMap<Uuid, Fm> {
        let mut hash = HashMap::new();
        hash.push(fm);
        hash
    }

    fn ids(&self) -> Vec<Uuid> {
        self.keys().cloned().collect()
    }

    fn push(&mut self, fm: Fm) {
        self.insert(fm.id(), fm);
    }

    fn stage(&self, staged: &HashMap<Uuid, Fm>) -> HashMap<Uuid, (Fm, StageSource)> {
        let mut result = HashMap::new();
        result.extend(
            self.clone()
                .into_iter()
                .map(|(id, file)| (id, (file, StageSource::Base))),
        );
        result.extend(
            staged
                .clone()
                .into_iter()
                .map(|(id, file)| (id, (file, StageSource::Staged))),
        );
        result
    }

    fn find(&self, id: Uuid) -> Result<Fm, TreeError> {
        self.get(&id).cloned().ok_or(FileNonexistent)
    }

    fn find_mut(&mut self, id: Uuid) -> Result<&mut Fm, TreeError> {
        self.maybe_find_mut(id).ok_or(FileNonexistent)
    }

    fn find_root(&self) -> Result<Fm, TreeError> {
        self.maybe_find_root().ok_or(RootNonexistent)
    }

    fn maybe_find_root(&self) -> Option<Fm> {
        self.values().find(|f| f.id() == f.parent()).cloned()
    }

    fn maybe_find(&self, id: Uuid) -> Option<Fm> {
        self.get(&id).cloned()
    }

    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut Fm> {
        self.get_mut(&id)
    }

    fn find_parent(&self, id: Uuid) -> Result<Fm, TreeError> {
        self.maybe_find_parent(id)
            .ok_or(TreeError::FileParentNonexistent)
    }

    fn maybe_find_parent(&self, id: Uuid) -> Option<Fm> {
        self.get(&self.get(&id)?.parent()).cloned()
    }

    fn find_children(&self, folder_id: Uuid) -> HashMap<Uuid, Fm> {
        self.iter()
            .filter_map(|(id, file)| {
                if file.parent() == folder_id && id != &file.parent() {
                    Some((*id, file.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn filter_deleted(&self) -> Result<HashMap<Uuid, Fm>, TreeError> {
        // todo!() //look into optimizing. Optimized: I think it's O(n) instead of O(n^2) now
        let mut result = HashMap::new();
        let mut not_deleted = HashMap::new();
        for (id, file) in self {
            let mut ancestors = HashMap::from([(*id, file.clone())]);
            let mut ancestor = file.clone();
            loop {
                if not_deleted.get(&ancestor.id()).is_none() // check it isn't confirmed as not deleted
                    && (ancestor.deleted() || result.get(&ancestor.id()).is_some())
                {
                    result.extend(ancestors);
                    break;
                }

                let parent = self.find(ancestor.parent())?;
                // first case is root, second case is a cycle (not our problem)
                if parent.id() == ancestor.id() || &parent.id() == id {
                    not_deleted.extend(ancestors);
                    break; // root
                }
                ancestors.insert(parent.id(), parent.clone());
                ancestor = parent;
            }
        }
        Ok(result)
    }

    fn filter_not_deleted(&self) -> Result<HashMap<Uuid, Fm>, TreeError> {
        // need rework, especially if allowed to change output of filter_deleted
        let deleted = self.filter_deleted()?;
        Ok(self
            .clone()
            .into_iter()
            .filter(|(id, _)| deleted.get(id).is_none())
            .collect())
    }

    fn filter_documents(&self) -> HashMap<Uuid, Fm> {
        self.clone()
            .into_iter()
            .filter(|(_, f)| f.file_type() == FileType::Document)
            .collect()
    }

    fn get_invalid_cycles(
        &self, staged_changes: &HashMap<Uuid, Fm>,
    ) -> Result<Vec<Uuid>, TreeError> {
        let root = match self.maybe_find_root() {
            Some(root) => Ok(root),
            None => staged_changes.find_root(),
        }?;
        let mut prev_checked = HashMap::new();
        let staged_changes = self.stage(staged_changes);
        let mut result = Vec::new();
        for (_, (f, _)) in staged_changes.clone().into_iter() {
            let mut checking = HashMap::new();
            let mut cur = &f;
            while cur.id() != root.id() && prev_checked.get(&cur.id()).is_none() {
                if checking.contains_key(&cur.id()) {
                    result.extend(checking.keys());
                    break;
                }
                checking.insert(cur.id(), cur.clone());
                cur = &staged_changes.get(&cur.parent()).unwrap().0;
            }
            prev_checked.extend(checking);
        }
        Ok(result)
    }

    fn get_path_conflicts(
        &self, staged_changes: &HashMap<Uuid, Fm>,
    ) -> Result<Vec<PathConflict>, TreeError> {
        let files_with_sources = self.stage(staged_changes);
        let mut name_tree: HashMap<Uuid, HashMap<Fm::Name, Uuid>> = HashMap::new();
        let mut result = Vec::new();

        for (id, (f, source)) in files_with_sources.iter() {
            let parent_id = f.parent();
            if id == &parent_id {
                continue;
            };
            let name = f.name();
            let parent_children = name_tree.entry(parent_id).or_insert_with(HashMap::new);
            let cloned_id = *id;

            if let Some(conflicting_child_id) = parent_children.get(&name) {
                match source {
                    StageSource::Base => result.push(PathConflict {
                        existing: cloned_id,
                        staged: conflicting_child_id.to_owned(),
                    }),
                    StageSource::Staged => result.push(PathConflict {
                        existing: conflicting_child_id.to_owned(),
                        staged: cloned_id,
                    }),
                }
            } else {
                parent_children.insert(name, cloned_id);
            };
        }
        Ok(result)
    }

    fn verify_integrity(&self) -> Result<(), TestFileTreeError> {
        if self.is_empty() {
            return Ok(());
        }

        if self.maybe_find_root().is_none() {
            return Err(TestFileTreeError::NoRootFolder);
        }

        for file in self.values() {
            if self.maybe_find(file.parent()).is_none() {
                return Err(TestFileTreeError::FileOrphaned(file.id()));
            }
        }

        let maybe_self_descendant = self
            .get_invalid_cycles(&HashMap::new())
            .map_err(TestFileTreeError::Tree)?
            .into_iter()
            .next();
        if let Some(self_descendant) = maybe_self_descendant {
            return Err(TestFileTreeError::CycleDetected(self_descendant));
        }

        let maybe_doc_with_children = self
            .filter_documents()
            .into_iter()
            .find(|(id, _)| !self.find_children(*id).is_empty());
        if let Some((id, _)) = maybe_doc_with_children {
            return Err(TestFileTreeError::DocumentTreatedAsFolder(id));
        }

        let maybe_path_conflict = self
            .get_path_conflicts(&HashMap::new())
            .map_err(TestFileTreeError::Tree)?
            .into_iter()
            .next();
        if let Some(path_conflict) = maybe_path_conflict {
            return Err(TestFileTreeError::NameConflictDetected(path_conflict.existing));
        }

        Ok(())
    }

    fn pretty_print(&self) -> String {
        fn print_branch<Fm: FileMetadata>(
            tree: &HashMap<Uuid, Fm>, file_leaf: &Fm, children: &HashMap<Uuid, Fm>, branch: &str,
            crotch: &str, twig: &str,
        ) -> String {
            let mut sub_tree = format!("{}{}{}\n", branch, twig, file_leaf);
            let mut next_branch = branch.to_string();
            next_branch.push_str(crotch);

            let num_children = children.len();

            for (count, (&id, child)) in children.iter().enumerate() {
                let next_children = tree.find_children(id);

                let last_child = count == num_children - 1;

                let next_crotch = if next_children.is_empty() {
                    ""
                } else if last_child {
                    "    "
                } else {
                    "│   "
                };

                let next_twig = if last_child { "└── " } else { "├── " };

                sub_tree.push_str(&print_branch(
                    tree,
                    child,
                    &next_children,
                    &next_branch,
                    next_crotch,
                    next_twig,
                ));
            }

            sub_tree
        }

        let root = match self.find_root() {
            Ok(root) => root,
            Err(_) => return "Failed to find root".to_string(),
        };
        print_branch(self, &root, &self.find_children(root.id()), "", "", "")
    }
}
