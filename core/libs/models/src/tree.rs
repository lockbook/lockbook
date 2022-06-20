use crate::file_metadata::FileType::{Document, Folder};
use crate::file_metadata::{FileType, Owner};
use crate::tree::TreeError::{FileNonexistent, RootNonexistent};
use std::collections::{HashMap, HashSet};
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

    fn is_root(&self) -> bool {
        self.id() == self.parent()
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

pub struct DeletedStatus {
    pub deleted: HashSet<Uuid>,
    pub not_deleted: HashSet<Uuid>,
}

pub trait FileMetaMapExt<Fm: FileMetadata> {
    fn with(fm: Fm) -> HashMap<Uuid, Fm>;
    fn ids(&self) -> Vec<Uuid>;
    fn push(&mut self, fm: Fm);
    fn stage(self, staged: HashMap<Uuid, Fm>) -> HashMap<Uuid, Fm>;
    fn stage_with_source(&self, staged: &HashMap<Uuid, Fm>) -> HashMap<Uuid, (Fm, StageSource)>;
    fn find(&self, id: Uuid) -> Result<Fm, TreeError>;
    fn find_ref(&self, id: Uuid) -> Result<&Fm, TreeError>;
    fn find_mut(&mut self, id: Uuid) -> Result<&mut Fm, TreeError>;
    fn find_root(&self) -> Result<Fm, TreeError>;
    fn maybe_find_root(&self) -> Option<Fm>;
    fn maybe_find(&self, id: Uuid) -> Option<Fm>;
    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut Fm>;
    fn find_parent(&self, id: Uuid) -> Result<&Fm, TreeError>;
    fn maybe_find_parent(&self, id: Uuid) -> Option<&Fm>;
    fn find_children(&self, id: Uuid) -> HashMap<Uuid, Fm>;
    fn filter_deleted(self) -> Result<HashMap<Uuid, Fm>, TreeError>;
    fn deleted_status(&self) -> Result<DeletedStatus, TreeError>;
    fn filter_not_deleted(self) -> Result<HashMap<Uuid, Fm>, TreeError>;
    fn documents(&self) -> HashSet<Uuid>;
    fn parents(&self) -> HashSet<Uuid>;
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

    fn stage(self, staged: HashMap<Uuid, Fm>) -> HashMap<Uuid, Fm> {
        let mut base = self;
        base.extend(staged);
        base
    }

    fn stage_with_source(&self, staged: &HashMap<Uuid, Fm>) -> HashMap<Uuid, (Fm, StageSource)> {
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

    fn find_ref(&self, id: Uuid) -> Result<&Fm, TreeError> {
        self.get(&id).ok_or(FileNonexistent)
    }

    fn find_mut(&mut self, id: Uuid) -> Result<&mut Fm, TreeError> {
        self.maybe_find_mut(id).ok_or(FileNonexistent)
    }

    fn find_root(&self) -> Result<Fm, TreeError> {
        self.maybe_find_root().ok_or(RootNonexistent)
    }

    fn maybe_find_root(&self) -> Option<Fm> {
        self.values().find(|f| f.is_root()).cloned()
    }

    fn maybe_find(&self, id: Uuid) -> Option<Fm> {
        self.get(&id).cloned()
    }

    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut Fm> {
        self.get_mut(&id)
    }

    fn find_parent(&self, id: Uuid) -> Result<&Fm, TreeError> {
        self.maybe_find_parent(id)
            .ok_or(TreeError::FileParentNonexistent)
    }

    fn maybe_find_parent(&self, id: Uuid) -> Option<&Fm> {
        self.get(&self.get(&id)?.parent())
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

    fn filter_deleted(self) -> Result<HashMap<Uuid, Fm>, TreeError> {
        let mut tree = self;
        for id in tree.deleted_status()?.not_deleted {
            tree.remove(&id);
        }

        Ok(tree)
    }

    fn deleted_status(&self) -> Result<DeletedStatus, TreeError> {
        let mut confirmed_not_delted = HashSet::new();
        let mut confirmed_deleted = HashSet::new();
        for meta in self.values() {
            // Itself is explicitly deleted
            if meta.deleted() {
                confirmed_deleted.insert(meta.id());
                continue;
            }

            // Parent is confirmed to be not deleted, and is not explicitly deleted
            if confirmed_not_delted.contains(&meta.parent()) && !meta.deleted() {
                confirmed_not_delted.insert(meta.id());
                continue;
            }

            // Check all ancestors
            let mut cur = self.find_ref(meta.parent())?;
            let mut checked_path = vec![meta.id(), cur.id()];
            let mut ancestor_deleted = false;
            'ansestor_check: while !cur.is_root() {
                if cur.deleted() {
                    confirmed_deleted.extend(&checked_path);
                    ancestor_deleted = true;
                    break 'ansestor_check;
                }

                // Select the next node to explore
                let parent = self.find_ref(cur.parent())?;
                if confirmed_deleted.contains(&parent.id()) {
                    confirmed_deleted.extend(&checked_path);
                    ancestor_deleted = true;
                    break 'ansestor_check;
                } else if confirmed_not_delted.contains(&parent.id()) {
                    confirmed_not_delted.extend(&checked_path);
                    break 'ansestor_check;
                } else {
                    if checked_path.contains(&parent.id()) {
                        // We've already checked this, it's likely a cycle or root, this is not the
                        // place to detect this though, cycles are not deleted files, so we continue
                        break 'ansestor_check;
                    }
                    checked_path.push(parent.id());
                    cur = parent;
                }
            }
            if !ancestor_deleted {
                confirmed_not_delted.extend(&checked_path);
            }
        }

        Ok(DeletedStatus { deleted: confirmed_deleted, not_deleted: confirmed_not_delted })
    }

    fn filter_not_deleted(self) -> Result<HashMap<Uuid, Fm>, TreeError> {
        let mut tree = self;
        for id in tree.deleted_status()?.deleted {
            tree.remove(&id);
        }

        Ok(tree)
    }

    fn documents(&self) -> HashSet<Uuid> {
        let mut result = HashSet::new();
        for meta in self.values() {
            if meta.is_document() {
                result.insert(meta.id());
            }
        }

        result
    }

    fn parents(&self) -> HashSet<Uuid> {
        let mut result = HashSet::new();
        for meta in self.values() {
            result.insert(meta.parent());
        }

        result
    }

    fn get_invalid_cycles(
        &self, staged_changes: &HashMap<Uuid, Fm>,
    ) -> Result<Vec<Uuid>, TreeError> {
        let mut root_found = false;
        let mut prev_checked = HashMap::new();
        let staged_changes = self.stage_with_source(staged_changes);
        let mut result = Vec::new();
        for (_, (f, _)) in staged_changes.clone().into_iter() {
            let mut checking = HashMap::new();
            let mut cur = &f;

            if cur.is_root() {
                if root_found {
                    result.push(cur.id());
                } else {
                    root_found = true;
                }
            }

            while !cur.is_root() && prev_checked.get(&cur.id()).is_none() {
                if checking.contains_key(&cur.id()) {
                    result.extend(checking.keys());
                    break;
                }
                checking.push(cur.clone());
                cur = &staged_changes.get(&cur.parent()).unwrap().0;
            }
            prev_checked.extend(checking);
        }
        Ok(result)
    }

    fn get_path_conflicts(
        &self, staged_changes: &HashMap<Uuid, Fm>,
    ) -> Result<Vec<PathConflict>, TreeError> {
        let files_with_sources = self.stage_with_source(staged_changes);
        let mut name_tree: HashMap<Uuid, HashMap<Fm::Name, Uuid>> = HashMap::new();
        let mut result = Vec::new();

        for (id, (f, source)) in files_with_sources.iter() {
            let parent_id = f.parent();
            if f.is_root() {
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
            if !self.contains_key(&file.parent()) {
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

        let docs = self.documents();
        let maybe_doc_with_children = self.parents().into_iter().find(|id| docs.contains(id));
        if let Some(id) = maybe_doc_with_children {
            // Could find all of them if we wanted to
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
