use crate::utils::StageSource;
use crate::{utils, CoreError};
use lockbook_crypto::symkey;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

pub fn create(file_type: FileType, parent: Uuid, name: &str, owner: &str) -> DecryptedFileMetadata {
    DecryptedFileMetadata {
        id: Uuid::new_v4(),
        file_type,
        parent,
        decrypted_name: String::from(name),
        owner: String::from(owner),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

pub fn create_root(username: &str) -> DecryptedFileMetadata {
    let id = Uuid::new_v4();
    DecryptedFileMetadata {
        id,
        file_type: FileType::Folder,
        parent: id,
        decrypted_name: String::from(username),
        owner: String::from(username),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

/// Validates a rename operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_rename(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
    new_name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = utils::find(files, target_id)?;
    validate_not_root(&file)?;
    validate_file_name(new_name)?;

    file.decrypted_name = String::from(new_name);
    if !get_path_conflicts(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a move operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_move(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
    new_parent: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = utils::find(files, target_id)?;
    let parent = utils::find_parent(files, target_id)?;
    validate_not_root(&file)?;
    validate_is_folder(&parent)?;

    file.parent = new_parent;
    if !get_path_conflicts(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }
    if !get_invalid_cycles(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::FolderMovedIntoSelf);
    }

    Ok(file)
}

/// Validates a delete operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_delete(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = utils::find(files, target_id)?;
    validate_not_root(&file)?;

    file.deleted = true;

    Ok(file)
}

fn validate_not_root(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.id != file.parent {
        Ok(())
    } else {
        Err(CoreError::RootModificationInvalid)
    }
}

fn validate_is_folder(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.file_type == FileType::Folder {
        Ok(())
    } else {
        Err(CoreError::FileNotFolder)
    }
}

fn validate_file_name(name: &str) -> Result<(), CoreError> {
    if name.is_empty() {
        return Err(CoreError::FileNameEmpty);
    }
    if name.contains('/') {
        return Err(CoreError::FileNameContainsSlash);
    }
    Ok(())
}

pub fn get_invalid_cycles_encrypted(
    files: &[FileMetadata],
    staged_changes: &[FileMetadata],
) -> Result<Vec<Uuid>, CoreError> {
    let files_with_sources = utils::stage_encrypted(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<FileMetadata>>();
    let mut result = Vec::new();

    'file_loop: for file in files {
        let mut ancestor = utils::find_parent_encrypted(files, file.id)?;

        if ancestor.id == file.id {
            continue; // root cycle is valid
        }

        while ancestor.id != file.id {
            ancestor = utils::find_parent_encrypted(files, ancestor.id)?;
            if ancestor.id == file.id {
                result.push(file.id); // non-root cycle is invalid
            }
            continue 'file_loop;
        }
    }

    Ok(result)
}

pub fn get_invalid_cycles(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Result<Vec<Uuid>, CoreError> {
    let files_with_sources = utils::stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<DecryptedFileMetadata>>();
    let mut result = Vec::new();

    'file_loop: for file in files {
        let mut ancestor = utils::find_parent(files, file.id)?;

        if ancestor.id == file.id {
            continue; // root cycle is valid
        }

        while ancestor.id != file.id {
            ancestor = utils::find_parent(files, ancestor.id)?;
            if ancestor.id == file.id {
                result.push(file.id); // non-root cycle is invalid
            }
            continue 'file_loop;
        }
    }

    Ok(result)
}

pub struct PathConflict {
    pub existing: Uuid,
    pub staged: Uuid,
}

pub fn get_path_conflicts(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Result<Vec<PathConflict>, CoreError> {
    let files_with_sources = utils::stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<DecryptedFileMetadata>>();
    let mut result = Vec::new();

    for file in files {
        let children = utils::find_children(files, file.id);
        let mut child_ids_by_name: HashMap<String, Uuid> = HashMap::new();
        for child in children {
            if let Some(conflicting_child_id) = child_ids_by_name.get(&child.decrypted_name) {
                let (_, child_source) = files_with_sources
                    .iter()
                    .find(|(f, _)| f.id == child.id)
                    .ok_or(CoreError::Unexpected(String::from(
                    "get_path_conflicts: could not find child by id",
                )))?;
                match child_source {
                    StageSource::Base => result.push(PathConflict {
                        existing: child.id,
                        staged: conflicting_child_id.to_owned(),
                    }),
                    StageSource::Staged => result.push(PathConflict {
                        existing: conflicting_child_id.to_owned(),
                        staged: child.id,
                    }),
                }
            }
            child_ids_by_name.insert(child.decrypted_name, child.id);
        }
    }

    Ok(result)
}

pub fn save_document_to_disk(document: &[u8], location: String) -> Result<(), CoreError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&location))
        .map_err(CoreError::from)?
        .write_all(document)
        .map_err(CoreError::from)?;
    Ok(())
}
