use crate::model::state::Config;
use crate::repo::file_metadata_repo;
use crate::service::file_service;
use crate::service::integrity_service::TestRepoError::{
    Core, CycleDetected, DocumentTreatedAsFolder, FileNameContainsSlash, FileNameEmpty,
    FileOrphaned, NameConflictDetected, NoRootFolder,
};
use crate::CoreError;
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use uuid::Uuid;

use super::file_encryption_service;
use crate::service::drawing_service::get_drawing;
use crate::service::path_service::get_path_by_id;

const UTF8_SUFFIXES: [&str; 12] = [
    "md", "txt", "text", "markdown", "sh", "zsh", "bash", "html", "css", "js", "csv", "rs",
];

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum Warning {
    EmptyFile(Uuid),
    InvalidUTF8(Uuid),
    UnreadableDrawing(Uuid),
}

pub fn test_repo_integrity(config: &Config) -> Result<Vec<Warning>, TestRepoError> {
    let root = file_metadata_repo::get_root(&config)
        .map_err(Core)?
        .ok_or(NoRootFolder)?;

    let all = file_metadata_repo::get_all(config).map_err(Core)?;

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

                match file_metadata_repo::maybe_get(&config, current.parent).map_err(Core)? {
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
                file_metadata_repo::get_children_non_recursively(&config, file.id).map_err(Core)?;
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

    let mut warnings = Vec::new();
    for file in all.clone() {
        if file.file_type == Document {
            let file_content = file_service::read_document(&config, file.id).map_err(Core)?;

            if file_content.len() as u64 == 0 {
                warnings.push(Warning::EmptyFile(file.id));
                continue;
            }

            let file_path = get_path_by_id(config, file.id).map_err(Core)?;
            let extension = Path::new(&file_path).extension().unwrap().to_str().unwrap();

            if UTF8_SUFFIXES.contains(&extension) && String::from_utf8(file_content).is_err() {
                warnings.push(Warning::InvalidUTF8(file.id));
                continue;
            }

            if extension == "draw" && get_drawing(config, file.id).is_err() {
                warnings.push(Warning::UnreadableDrawing(file.id));
            }
        }
    }

    Ok(warnings)
}
