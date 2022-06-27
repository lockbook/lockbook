use libsecp256k1::PublicKey;

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_crypto::symkey;
use lockbook_models::account::Account;
use lockbook_models::file_metadata::{DecryptedFileMetadata, DecryptedFiles, FileType, Owner};
use lockbook_models::tree::{FileMetaMapExt, FileMetaVecExt, FileMetadata};

use crate::model::filename::NameComponents;
use crate::{model::repo::RepoState, CoreError};

pub fn create(
    file_type: FileType, parent: Uuid, name: &str, owner: &PublicKey,
) -> DecryptedFileMetadata {
    DecryptedFileMetadata {
        id: Uuid::new_v4(),
        file_type,
        parent,
        decrypted_name: String::from(name),
        owner: Owner(*owner),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

pub fn create_root(account: &Account) -> DecryptedFileMetadata {
    let id = Uuid::new_v4();
    DecryptedFileMetadata {
        id,
        file_type: FileType::Folder,
        parent: id,
        decrypted_name: account.username.clone(),
        owner: Owner::from(account),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

/// Validates a create operation for a file in the context of all files and returns a version of
/// the file with the operation applied. This is a pure function.
pub fn apply_create(
    files: &DecryptedFiles, file_type: FileType, parent: Uuid, name: &str, owner: &PublicKey,
) -> Result<DecryptedFileMetadata, CoreError> {
    let file = create(file_type, parent, name, owner);
    validate_not_root(&file)?;
    validate_file_name(name)?;
    let parent = files
        .maybe_find(parent)
        .ok_or(CoreError::FileParentNonexistent)?;
    validate_is_folder(&parent)?;

    let staged_changes = HashMap::with(file.clone());
    if !files.get_path_conflicts(&staged_changes)?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a rename operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_rename(
    files: &DecryptedFiles, target_id: Uuid, new_name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
    validate_not_root(&file)?;
    validate_file_name(new_name)?;

    file.decrypted_name = String::from(new_name);
    if !files
        .get_path_conflicts(&[file.clone()].to_map())?
        .is_empty()
    {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a move operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_move(
    files: &DecryptedFiles, target_id: Uuid, new_parent: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
    let parent = files
        .maybe_find(new_parent)
        .ok_or(CoreError::FileParentNonexistent)?;
    validate_not_root(&file)?;
    validate_is_folder(&parent)?;

    file.parent = new_parent;
    let staged_changes = HashMap::with(file.clone());
    if !files.get_invalid_cycles(&staged_changes)?.is_empty() {
        return Err(CoreError::FolderMovedIntoSelf);
    }
    if !files.get_path_conflicts(&staged_changes)?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a delete operation for a file in the context of all files and returns a version of the
/// file with the operation applied. This is a pure function.
pub fn apply_delete(
    files: &DecryptedFiles, target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
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
    if file.is_folder() {
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

pub fn suggest_non_conflicting_filename(
    id: Uuid, files: &DecryptedFiles, staged_changes: &DecryptedFiles,
) -> Result<String, CoreError> {
    let files: DecryptedFiles = files
        .stage_with_source(staged_changes)
        .into_iter()
        .map(|(id, (f, _))| (id, f))
        .collect::<DecryptedFiles>();

    let file = files.find(id)?;
    let sibblings = files.find_children(file.parent);

    let mut new_name = NameComponents::from(&file.decrypted_name).generate_next();
    loop {
        if !sibblings
            .values()
            .any(|f| f.decrypted_name == new_name.to_name())
        {
            return Ok(new_name.to_name());
        } else {
            new_name = new_name.generate_next();
        }
    }
}

// TODO this is not a pure function and should be relocated
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

pub fn find_state<Fm: FileMetadata>(
    files: &[RepoState<Fm>], target_id: Uuid,
) -> Result<RepoState<Fm>, CoreError> {
    maybe_find_state(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn maybe_find_state<Fm: FileMetadata>(
    files: &[RepoState<Fm>], target_id: Uuid,
) -> Option<RepoState<Fm>> {
    files.iter().find(|f| match f {
        RepoState::New(l) => l.id(),
        RepoState::Modified { local: l, base: _ } => l.id(),
        RepoState::Unmodified(b) => b.id(),
    } == target_id).cloned()
}

pub fn find_ancestors<Fm: FileMetadata>(
    files: &HashMap<Uuid, Fm>, target_id: Uuid,
) -> HashMap<Uuid, Fm> {
    let mut result = HashMap::new();
    let mut current_target_id = target_id;
    while let Some(target) = files.maybe_find(current_target_id) {
        result.push(target.clone());
        if target.is_root() {
            break;
        }
        current_target_id = target.parent();
    }
    result
}

pub fn find_children<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Vec<Fm> {
    files
        .iter()
        .filter(|f| f.parent() == target_id && f.id() != f.parent())
        .cloned()
        .collect()
}

pub fn find_with_descendants<Fm: FileMetadata>(
    files: &HashMap<Uuid, Fm>, target_id: Uuid,
) -> Result<HashMap<Uuid, Fm>, CoreError> {
    let mut result = HashMap::new();
    let mut unexplored: HashMap<Uuid, Fm> = HashMap::new();
    unexplored.push(files.find(target_id)?);
    while !unexplored.is_empty() {
        let mut next_exploration = HashMap::new();
        for file in unexplored.values() {
            result.push(file.clone());
            if file.is_folder() {
                next_exploration.extend(files.find_children(file.id()));
            }
        }
        unexplored = next_exploration;
    }

    Ok(result)
}
