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
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeError {
    RootNonexistent,
    FileNonexistent,
    FileParentNonexistent,
    Unexpected(String),
}

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

pub trait FileMetaExt<T: FileMetadata> {
    fn ids(&self) -> Vec<Uuid>;
    fn stage(&self, staged: &[T]) -> Vec<(T, StageSource)>;
    fn find(&self, id: Uuid) -> Result<T, TreeError>;
    fn find_mut(&mut self, id: Uuid) -> Result<&mut T, TreeError>;
    fn find_root(&self) -> Result<T, TreeError>;
    fn maybe_find_root(&self) -> Option<T>;
    fn maybe_find(&self, id: Uuid) -> Option<T>;
    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut T>;
    fn find_parent(&self, id: Uuid) -> Result<T, TreeError>;
    fn maybe_find_parent(&self, id: Uuid) -> Option<T>;
    fn find_children(&self, id: Uuid) -> Vec<T>;
    fn filter_deleted(&self) -> Result<Vec<T>, TreeError>;
    fn filter_not_deleted(&self) -> Result<Vec<T>, TreeError>;
    fn filter_documents(&self) -> Vec<T>;
    fn get_invalid_cycles(&self, staged_changes: &[T]) -> Result<Vec<Uuid>, TreeError>;
    fn get_path_conflicts(&self, staged_changes: &[T]) -> Result<Vec<PathConflict>, TreeError>;
    fn verify_integrity(&self) -> Result<(), TestFileTreeError>;
    fn pretty_print(&self) -> String;
}

impl<Fm> FileMetaExt<Fm> for [Fm]
where
    Fm: FileMetadata,
{
    fn ids(&self) -> Vec<Uuid> {
        self.iter().map(|f| f.id()).collect()
    }

    fn stage(&self, staged: &[Fm]) -> Vec<(Fm, StageSource)> {
        let mut result = Vec::new();
        for file in self {
            if let Some(ref staged) = staged.maybe_find(file.id()) {
                result.push((staged.clone(), StageSource::Staged));
            } else {
                result.push((file.clone(), StageSource::Base));
            }
        }
        for staged in staged {
            if self.maybe_find(staged.id()).is_none() {
                result.push((staged.clone(), StageSource::Staged));
            }
        }
        result
    }

    fn find(&self, id: Uuid) -> Result<Fm, TreeError> {
        self.maybe_find(id).ok_or(FileNonexistent)
    }

    fn find_mut(&mut self, id: Uuid) -> Result<&mut Fm, TreeError> {
        self.maybe_find_mut(id).ok_or(FileNonexistent)
    }

    fn find_root(&self) -> Result<Fm, TreeError> {
        self.maybe_find_root().ok_or(RootNonexistent)
    }

    fn maybe_find_root(&self) -> Option<Fm> {
        self.iter().find(|f| f.id() == f.parent()).cloned()
    }

    fn maybe_find(&self, id: Uuid) -> Option<Fm> {
        self.iter().find(|f| f.id() == id).cloned()
    }
    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut Fm> {
        self.iter_mut().find(|f| f.id() == id)
    }

    fn find_parent(&self, id: Uuid) -> Result<Fm, TreeError> {
        self.maybe_find_parent(id)
            .ok_or(TreeError::FileParentNonexistent)
    }

    fn maybe_find_parent(&self, id: Uuid) -> Option<Fm> {
        self.maybe_find(self.maybe_find(id)?.parent())
    }

    fn find_children(&self, id: Uuid) -> Vec<Fm> {
        self.iter()
            .filter(|f| f.parent() == id && f.id() != f.parent())
            .cloned()
            .collect()
    }

    /// Returns the files which are deleted or have deleted ancestors. It is an error for the parent
    /// of a file argument not to also be included in the arguments.
    fn filter_deleted(&self) -> Result<Vec<Fm>, TreeError> {
        let mut result = Vec::new();
        for file in self {
            let mut ancestor = file.clone();
            loop {
                if ancestor.deleted() {
                    result.push(file.clone());
                    break;
                }

                let parent = self.find(ancestor.parent())?;
                if ancestor.id() == parent.id() {
                    break;
                }
                ancestor = parent;
                if ancestor.id() == file.id() {
                    break; // this is a cycle but not our problem
                }
            }
        }
        Ok(result)
    }

    /// Returns the files which are not deleted and have no deleted ancestors. It is an error for
    /// the parent of a file argument not to also be included in the arguments.
    fn filter_not_deleted(&self) -> Result<Vec<Fm>, TreeError> {
        let deleted = self.filter_deleted()?;
        Ok(self
            .iter()
            .filter(|f| !deleted.iter().any(|nd| nd.id() == f.id()))
            .cloned()
            .collect())
    }

    /// Returns the files which are documents.
    fn filter_documents(&self) -> Vec<Fm> {
        self.iter()
            .filter(|f| f.file_type() == FileType::Document)
            .cloned()
            .collect()
    }

    fn get_invalid_cycles(&self, staged_changes: &[Fm]) -> Result<Vec<Uuid>, TreeError> {
        let maybe_root = self.maybe_find_root();
        let files_with_sources = self.stage(staged_changes);
        let files = &files_with_sources
            .iter()
            .map(|(f, _)| f.clone())
            .collect::<Vec<Fm>>();
        let mut result = Vec::new();
        let mut found_root = maybe_root.is_some();

        for file in files {
            let mut ancestor_single = files.find_parent(file.id())?;
            let mut ancestor_double = files.find_parent(ancestor_single.id())?;
            while ancestor_single.id() != ancestor_double.id() {
                ancestor_single = files.find_parent(ancestor_single.id())?;
                ancestor_double =
                    files.find_parent(files.find_parent(ancestor_double.id())?.id())?;
            }
            if ancestor_single.id() == file.id() {
                // root in files -> non-root cycles invalid
                // no root in files -> accept first root from staged_changes
                if let Some(ref root) = maybe_root {
                    if file.id() != root.id() {
                        result.push(file.id());
                    }
                } else if !found_root {
                    found_root = true;
                } else {
                    result.push(file.id());
                }
            }
        }

        Ok(result)
    }

    fn get_path_conflicts(&self, staged_changes: &[Fm]) -> Result<Vec<PathConflict>, TreeError> {
        let files_with_sources = self.stage(staged_changes);
        let files = &files_with_sources
            .iter()
            .map(|(f, _)| f.clone())
            .collect::<Vec<Fm>>();
        let files = files.filter_not_deleted()?;
        let mut result = Vec::new();

        for file in &files {
            let children = files.find_children(file.id());
            let mut child_ids_by_name: HashMap<Fm::Name, Uuid> = HashMap::new();
            for child in children {
                if let Some(conflicting_child_id) = child_ids_by_name.get(&child.name()) {
                    let (_, child_source) = files_with_sources
                        .iter()
                        .find(|(f, _)| f.id() == child.id())
                        .ok_or_else(|| {
                            TreeError::Unexpected(String::from(
                                "get_path_conflicts: could not find child by id",
                            ))
                        })?;
                    match child_source {
                        StageSource::Base => result.push(PathConflict {
                            existing: child.id(),
                            staged: conflicting_child_id.to_owned(),
                        }),
                        StageSource::Staged => result.push(PathConflict {
                            existing: conflicting_child_id.to_owned(),
                            staged: child.id(),
                        }),
                    }
                }
                child_ids_by_name.insert(child.name(), child.id());
            }
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

        for file in self {
            if self.maybe_find(file.parent()).is_none() {
                return Err(TestFileTreeError::FileOrphaned(file.id()));
            }
        }

        let maybe_self_descendant = self
            .get_invalid_cycles(&[])
            .map_err(TestFileTreeError::Tree)?
            .into_iter()
            .next();
        if let Some(self_descendant) = maybe_self_descendant {
            return Err(TestFileTreeError::CycleDetected(self_descendant));
        }

        let maybe_doc_with_children = self
            .filter_documents()
            .into_iter()
            .find(|doc| !self.find_children(doc.id()).is_empty());
        if let Some(doc) = maybe_doc_with_children {
            return Err(TestFileTreeError::DocumentTreatedAsFolder(doc.id()));
        }

        let maybe_path_conflict = self
            .get_path_conflicts(&[])
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
            tree: &[Fm], file_leaf: &Fm, children: &[Fm], branch: &str, crotch: &str, twig: &str,
        ) -> String {
            let mut sub_tree = format!("{}{}{}\n", branch, twig, file_leaf);
            let mut next_branch = branch.to_string();
            next_branch.push_str(crotch);

            let num_children = children.len();

            for (count, child) in children.iter().enumerate() {
                let next_children = tree.find_children(child.id());

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

impl<Fm> FileMetaExt<Fm> for HashMap<Uuid, Fm>
where
    Fm: FileMetadata,
{
    fn ids(&self) -> Vec<Uuid> {
        self.keys().map(|k| *k).collect()
    }

    fn stage(&self, staged: &[Fm]) -> Vec<(Fm, StageSource)> {
        let mut result = Vec::new();
        for (_, file) in self {
            if let Some(ref staged) = staged.maybe_find(file.id()) {
                result.push((staged.clone(), StageSource::Staged));
            } else {
                result.push((file.clone(), StageSource::Base));
            }
        }
        for staged in staged {
            if self.maybe_find(staged.id()).is_none() {
                result.push((staged.clone(), StageSource::Staged));
            }
        }
        result
    }

    fn find(&self, id: Uuid) -> Result<Fm, TreeError> {
        match self.get(&id).cloned() {
            Some(file_metadata) => Ok(file_metadata),
            None => Err(FileNonexistent),
        }
    }

    fn find_mut(&mut self, id: Uuid) -> Result<&mut Fm, TreeError> {
        self.maybe_find_mut(id).ok_or(FileNonexistent)
    }

    fn find_root(&self) -> Result<Fm, TreeError> {
        self.maybe_find_root().ok_or(RootNonexistent)
    }

    fn maybe_find_root(&self) -> Option<Fm> {
        self.iter()
            .find_map(|(&id, &f)| if id == f.parent() { Some(&f) } else { None })
            .cloned()
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

    fn find_children(&self, id: Uuid) -> Vec<Fm> {
        self.iter()
            .filter_map(
                |(f_id, f)| if f.parent() == id && f_id != &f.parent() { Some(f) } else { None },
            )
            .cloned()
            .collect()
    }

    fn filter_deleted(&self) -> Result<Vec<Fm>, TreeError> {
        todo!() //look into optimizing
    }

    fn filter_not_deleted(&self) -> Result<Vec<Fm>, TreeError> {
        todo!()
    }

    fn filter_documents(&self) -> Vec<Fm> {
        todo!()
    }

    fn get_invalid_cycles(&self, staged_changes: &[Fm]) -> Result<Vec<Uuid>, TreeError> {
        todo!()
    }

    fn get_path_conflicts(&self, staged_changes: &[Fm]) -> Result<Vec<PathConflict>, TreeError> {
        todo!()
    }

    fn verify_integrity(&self) -> Result<(), TestFileTreeError> {
        todo!()
    }

    fn pretty_print(&self) -> String {
        todo!()
    }
}
// impl<Fm> FileMetaExt<Fm> for HashMap<Uuid,Fm>
// where
//     Fm: FileMetadata,
// {
//     fn ids(&self) -> Vec<Uuid> {
//         self.iter().map(|(u,f)| f.id()).collect()
//     }
//
//     fn stage(&self, staged: &[Fm]) -> Vec<(Fm, StageSource)> {
//         let mut result = Vec::new();
//         for (id, file) in self {
//             if let Some(ref staged) = staged.maybe_find(file.id()) {
//                 result.push((staged.clone(), StageSource::Staged));
//             } else {
//                 result.push((file.clone(), StageSource::Base));
//             }
//         }
//         for staged in staged {
//             if self.maybe_find(staged.id()).is_none() {
//                 result.push((staged.clone(), StageSource::Staged));
//             }
//         }
//         result
//     }
//
//     fn find(&self, id: Uuid) -> Result<Fm, TreeError> {
//         self.maybe_find(id).ok_or(FileNonexistent)
//     }
//
//     fn find_mut(&mut self, id: Uuid) -> Result<&mut Fm, TreeError> {
//         self.maybe_find_mut(id).ok_or(TreeError::FileNonexistent)
//     }
//
//     fn find_root(&self) -> Result<Fm, TreeError> {
//         self.maybe_find_root().ok_or(RootNonexistent)
//     }
//
//     fn maybe_find_root(&self) -> Option<Fm> {
//         self.iter().find(|(id, f)| f.id() == f.parent())[1].cloned()
//     }
//
//     fn maybe_find(&self, id: Uuid) -> Option<Fm> {
//         self.iter().find(|(id, f)| f.id() == id).cloned()
//     }
//     fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut Fm> {
//         self.iter_mut().find(|(id, f)| f.id() == id)
//     }
//
//     fn find_parent(&self, id: Uuid) -> Result<Fm, TreeError> {
//         self.maybe_find_parent(id)
//             .ok_or(TreeError::FileParentNonexistent)
//     }
//
//     fn maybe_find_parent(&self, id: Uuid) -> Option<Fm> {
//         self.maybe_find(self.maybe_find(id)?.parent())
//     }
//
//     fn find_children(&self, id: Uuid) -> Vec<Fm> {
//         self.iter()
//             .filter(|(child_id, f)| f.parent() == id && child_id != f.parent())
//             .cloned()
//             .collect()
//     }
//
//     /// Returns the files which are deleted or have deleted ancestors. It is an error for the parent
//     /// of a file argument not to also be included in the arguments.
//     fn filter_deleted(&self) -> Result<Vec<Fm>, TreeError> {
//         let mut result = Vec::new();
//         for file in self {
//             let mut ancestor = file.clone();
//             loop {
//                 if ancestor.deleted() {
//                     result.push(file.clone());
//                     break;
//                 }
//
//                 let parent = self.find(ancestor.parent())?;
//                 if ancestor.id() == parent.id() {
//                     break;
//                 }
//                 ancestor = parent;
//                 if ancestor.id() == file.id() {
//                     break; // this is a cycle but not our problem
//                 }
//             }
//         }
//         Ok(result)
//     }
//
//     /// Returns the files which are not deleted and have no deleted ancestors. It is an error for
//     /// the parent of a file argument not to also be included in the arguments.
//     fn filter_not_deleted(&self) -> Result<Vec<Fm>, TreeError> {
//         let deleted = self.filter_deleted()?;
//         Ok(self
//             .iter()
//             .filter(|f| !deleted.iter().any(|nd| nd.id() == f.id()))
//             .cloned()
//             .collect())
//     }
//
//     /// Returns the files which are documents.
//     fn filter_documents(&self) -> Vec<Fm> {
//         self.iter()
//             .filter(|f| f.file_type() == FileType::Document)
//             .cloned()
//             .collect()
//     }
//
//     fn get_invalid_cycles(&self, staged_changes: &[Fm]) -> Result<Vec<Uuid>, TreeError> {
//         let maybe_root = self.maybe_find_root();
//         let files_with_sources = self.stage(staged_changes);
//         let files = &files_with_sources
//             .iter()
//             .map(|(f, _)| f.clone())
//             .collect::<Vec<Fm>>();
//         let mut result = Vec::new();
//         let mut found_root = maybe_root.is_some();
//
//         for file in files {
//             let mut ancestor_single = files.find_parent(file.id())?;
//             let mut ancestor_double = files.find_parent(ancestor_single.id())?;
//             while ancestor_single.id() != ancestor_double.id() {
//                 ancestor_single = files.find_parent(ancestor_single.id())?;
//                 ancestor_double =
//                     files.find_parent(files.find_parent(ancestor_double.id())?.id())?;
//             }
//             if ancestor_single.id() == file.id() {
//                 // root in files -> non-root cycles invalid
//                 // no root in files -> accept first root from staged_changes
//                 if let Some(ref root) = maybe_root {
//                     if file.id() != root.id() {
//                         result.push(file.id());
//                     }
//                 } else if !found_root {
//                     found_root = true;
//                 } else {
//                     result.push(file.id());
//                 }
//             }
//         }
//
//         Ok(result)
//     }
//
//     fn get_path_conflicts(&self, staged_changes: &[Fm]) -> Result<Vec<PathConflict>, TreeError> {
//         let files_with_sources = self.stage(staged_changes);
//         let files = &files_with_sources
//             .iter()
//             .map(|(f, _)| f.clone())
//             .collect::<Vec<Fm>>();
//         let files = files.filter_not_deleted()?;
//         let mut result = Vec::new();
//
//         for file in &files {
//             let children = files.find_children(file.id());
//             let mut child_ids_by_name: HashMap<Fm::Name, Uuid> = HashMap::new();
//             for child in children {
//                 if let Some(conflicting_child_id) = child_ids_by_name.get(&child.name()) {
//                     let (_, child_source) = files_with_sources
//                         .iter()
//                         .find(|(f, _)| f.id() == child.id())
//                         .ok_or_else(|| {
//                             TreeError::Unexpected(String::from(
//                                 "get_path_conflicts: could not find child by id",
//                             ))
//                         })?;
//                     match child_source {
//                         StageSource::Base => result.push(PathConflict {
//                             existing: child.id(),
//                             staged: conflicting_child_id.to_owned(),
//                         }),
//                         StageSource::Staged => result.push(PathConflict {
//                             existing: conflicting_child_id.to_owned(),
//                             staged: child.id(),
//                         }),
//                     }
//                 }
//                 child_ids_by_name.insert(child.name(), child.id());
//             }
//         }
//
//         Ok(result)
//     }
//
//     fn verify_integrity(&self) -> Result<(), TestFileTreeError> {
//         if self.is_empty() {
//             return Ok(());
//         }
//
//         if self.maybe_find_root().is_none() {
//             return Err(TestFileTreeError::NoRootFolder);
//         }
//
//         for file in self {
//             if self.maybe_find(file.parent()).is_none() {
//                 return Err(TestFileTreeError::FileOrphaned(file.id()));
//             }
//         }
//
//         let maybe_self_descendant = self
//             .get_invalid_cycles(&[])
//             .map_err(TestFileTreeError::Tree)?
//             .into_iter()
//             .next();
//         if let Some(self_descendant) = maybe_self_descendant {
//             return Err(TestFileTreeError::CycleDetected(self_descendant));
//         }
//
//         let maybe_doc_with_children = self
//             .filter_documents()
//             .into_iter()
//             .find(|doc| !self.find_children(doc.id()).is_empty());
//         if let Some(doc) = maybe_doc_with_children {
//             return Err(TestFileTreeError::DocumentTreatedAsFolder(doc.id()));
//         }
//
//         let maybe_path_conflict = self
//             .get_path_conflicts(&[])
//             .map_err(TestFileTreeError::Tree)?
//             .into_iter()
//             .next();
//         if let Some(path_conflict) = maybe_path_conflict {
//             return Err(TestFileTreeError::NameConflictDetected(path_conflict.existing));
//         }
//
//         Ok(())
//     }
//
//     fn pretty_print(&self) -> String {
//         fn print_branch<Fm: FileMetadata>(
//             tree: &[Fm], file_leaf: &Fm, children: &[Fm], branch: &str, crotch: &str, twig: &str,
//         ) -> String {
//             let mut sub_tree = format!("{}{}{}\n", branch, twig, file_leaf);
//             let mut next_branch = branch.to_string();
//             next_branch.push_str(crotch);
//
//             let num_children = children.len();
//
//             for (count, child) in children.iter().enumerate() {
//                 let next_children = tree.find_children(child.id());
//
//                 let last_child = count == num_children - 1;
//
//                 let next_crotch = if next_children.is_empty() {
//                     ""
//                 } else if last_child {
//                     "    "
//                 } else {
//                     "│   "
//                 };
//
//                 let next_twig = if last_child { "└── " } else { "├── " };
//
//                 sub_tree.push_str(&print_branch(
//                     tree,
//                     child,
//                     &next_children,
//                     &next_branch,
//                     next_crotch,
//                     next_twig,
//                 ));
//             }
//
//             sub_tree
//         }
//
//         let root = match self.find_root() {
//             Ok(root) => root,
//             Err(_) => return "Failed to find root".to_string(),
//         };
//         print_branch(self, &root, &self.find_children(root.id()), "", "", "")
//     }
// }
