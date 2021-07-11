use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::file_repo;
use crate::service::integrity_service::TestRepoError::{
    Core, CycleDetected, DocumentTreatedAsFolder, FileNameContainsSlash, FileNameEmpty,
    FileOrphaned, NameConflictDetected, NoRootFolder,
};
use crate::CoreError;
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::file_encryption_service;

#[derive(Debug)]
pub enum TestRepoError {
    NoRootFolder,
    DocumentTreatedAsFolder(Uuid),
    FileOrphaned(Uuid),
    CycleDetected(Uuid),
    FileNameEmpty(Uuid),
    FileNameContainsSlash(Uuid),
    NameConflictDetected(Uuid),
    Core(CoreError),
}

pub fn test_repo_integrity(config: &Config) -> Result<(), TestRepoError> {
    let root = file_repo::maybe_get_root(&config, RepoSource::Local)
        .map_err(Core)?
        .ok_or(NoRootFolder)?;

    let all = file_repo::get_all_metadata(config, RepoSource::Local).map_err(Core)?;

    {
        let document_with_children = all
            .clone()
            .into_iter()
            .filter(|f| f.file_type == Document)
            .filter(|doc| all.clone().into_iter().any(|child| child.parent == doc.id))
            .last();

        if let Some(file) = document_with_children {
            return Err(DocumentTreatedAsFolder(file.id));
        }
    }

    // Find files that don't descend from root
    // todo: file_repo::get_all_metadata is implemented as only getting root and its descendants, so this would never catch an issue
    {
        let mut not_orphaned = HashMap::new();
        not_orphaned.insert(root.id, root);

        for file in all.clone() {
            let mut visited: HashMap<Uuid, FileMetadata> = HashMap::new();
            let mut current = file.clone();
            'parent_finder: loop {
                if visited.contains_key(&current.id) {
                    return Err(CycleDetected(current.id));
                }
                visited.insert(current.id, current.clone());

                match file_repo::maybe_get_metadata(&config, RepoSource::Local, current.parent)
                    .map_err(Core)?
                {
                    None => {
                        return Err(FileOrphaned(current.id));
                    }
                    Some(parent) => {
                        // No Problems
                        if not_orphaned.contains_key(&parent.id) {
                            for node in visited.values() {
                                not_orphaned.insert(node.id, node.clone());
                            }

                            break 'parent_finder;
                        } else {
                            current = parent.clone();
                        }
                    }
                }
            }
        }
    }

    // Find files with invalid names
    for file in all.clone() {
        let name = file_encryption_service::get_name(&config, &file).map_err(Core)?;
        if name.is_empty() {
            return Err(FileNameEmpty(file.id));
        }

        if name.contains('/') {
            return Err(FileNameContainsSlash(file.id));
        }
    }

    // Find naming conflicts
    {
        for file in all.iter().filter(|f| f.file_type == Folder) {
            let children =
                file_repo::get_children(&config, RepoSource::Local, file.id).map_err(Core)?;
            let mut children_set = HashSet::new();
            for child in children {
                let name = file_encryption_service::get_name(&config, &child).map_err(Core)?;
                if children_set.contains(&name) {
                    return Err(NameConflictDetected(child.id));
                }

                children_set.insert(name);
            }
        }
    }

    Ok(())
}
