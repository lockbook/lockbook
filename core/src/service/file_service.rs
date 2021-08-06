use crate::model::client_conversion::{self, ClientFileMetadata};
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::{account_repo, file_repo, metadata_repo};
use crate::service::file_compression_service;
use crate::service::file_encryption_service;
use crate::CoreError;
use lockbook_crypto::symkey;
use lockbook_models::crypto::{DecryptedDocument, EncryptedDocument};
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

pub fn create_2(
    file_type: FileType,
    parent: Uuid,
    name: &str,
    owner: &str,
) -> DecryptedFileMetadata {
    DecryptedFileMetadata {
        id: Uuid::new_v4(),
        file_type,
        parent,
        decrypted_name: String::from(name),
        owner: String::from(owner),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_user_access_key: None,
        decrypted_folder_access_keys: symkey::generate_key(),
    }
}

pub fn create_root(username: &str) -> DecryptedFileMetadata {
    let id = Uuid::new_v4();
    let key = symkey::generate_key();
    DecryptedFileMetadata {
        id,
        file_type: FileType::Folder,
        parent: id,
        decrypted_name: String::from(username),
        owner: String::from(username),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_user_access_key: Some(key),
        decrypted_folder_access_keys: key,
    }
}

// todo: make a place for this (utils?)
pub fn find(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    maybe_find(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn maybe_find(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Option<DecryptedFileMetadata> {
    files.iter().find(|f| f.id == target_id).map(|f| f.clone())
}

pub fn find_parent(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    maybe_find_parent(files, target_id).ok_or(CoreError::FileParentNonexistent)
}

pub fn maybe_find_parent(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Option<DecryptedFileMetadata> {
    let file = maybe_find(files, target_id)?;
    maybe_find(files, file.parent)
}

pub fn find_children(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Vec<DecryptedFileMetadata> {
    files
        .iter()
        .filter(|f| f.parent == target_id)
        .map(|f| f.clone())
        .collect()
}

pub fn find_root(files: &[DecryptedFileMetadata]) -> Result<DecryptedFileMetadata, CoreError> {
    maybe_find_root(files).ok_or(CoreError::RootNonexistent)
}

pub fn maybe_find_root(files: &[DecryptedFileMetadata]) -> Option<DecryptedFileMetadata> {
    files.iter().find(|f| f.id == f.parent).map(|f| f.clone())
}

/// Validates a rename operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_rename(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
    new_name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = find(files, target_id)?;
    validate_not_root_2(&file)?;
    validate_file_name_2(new_name)?;

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
    let mut file = find(files, target_id)?;
    let parent = find_parent(files, target_id)?;
    validate_not_root_2(&file)?;
    validate_is_folder_2(&parent)?;

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
    let mut file = find(files, target_id)?;
    validate_not_root_2(&file)?;

    file.deleted = true;

    Ok(file)
}

fn validate_not_root_2(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.id != file.parent {
        Ok(())
    } else {
        Err(CoreError::RootModificationInvalid)
    }
}

fn validate_is_folder_2(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.file_type == FileType::Folder {
        Ok(())
    } else {
        Err(CoreError::FileNotFolder)
    }
}

fn validate_file_name_2(name: &str) -> Result<(), CoreError> {
    if name.is_empty() {
        return Err(CoreError::FileNameEmpty);
    }
    if name.contains('/') {
        return Err(CoreError::FileNameContainsSlash);
    }
    Ok(())
}

pub enum StageSource {
    Base,
    Staged,
}

pub fn stage(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Vec<(DecryptedFileMetadata, StageSource)> {
    let mut result = Vec::new();
    for file in files {
        if let Some(ref staged) = maybe_find(staged_changes, file.id) {
            result.push((staged.clone(), StageSource::Staged));
        } else {
            result.push((file.clone(), StageSource::Base));
        }
    }
    for staged in staged_changes {
        if maybe_find(files, staged.id).is_none() {
            result.push((staged.clone(), StageSource::Staged));
        }
    }
    result
}

pub fn get_invalid_cycles(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Result<Vec<Uuid>, CoreError> {
    let files_with_sources = stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<DecryptedFileMetadata>>();
    let mut result = Vec::new();

    'file_loop: for file in files {
        let mut ancestor = find_parent(files, file.id)?;

        if ancestor.id == file.id {
            continue; // root cycle is valid
        }

        while ancestor.id != file.id {
            ancestor = find_parent(files, ancestor.id)?;
            if ancestor.id == file.id {
                result.push(file.id); // non-root cycle is invalid
            }
            continue 'file_loop;
        }
    }

    Ok(result)
}

pub struct PathConflict {
    existing: Uuid,
    staged: Uuid,
}

pub fn get_path_conflicts(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Result<Vec<PathConflict>, CoreError> {
    let files_with_sources = stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<DecryptedFileMetadata>>();
    let mut result = Vec::new();

    for file in files {
        let children = find_children(files, file.id);
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

// ------------------------------------------------------------------------------------------------------------------- new ^ / v old

pub fn create(
    config: &Config,
    source: RepoSource,
    name: &str,
    parent: Uuid,
    file_type: FileType,
) -> Result<FileMetadata, CoreError> {
    validate_file_name(name)?;
    account_repo::get(config)?;

    let file_metadata =
        file_encryption_service::create_file_metadata(&config, name, file_type, parent)?;
    insert_metadata(config, source, &file_metadata)?;

    Ok(file_metadata)
}

pub fn insert_metadata(
    config: &Config,
    source: RepoSource,
    file_metadata: &FileMetadata,
) -> Result<(), CoreError> {
    validate_not_own_ancestor(config, source, &file_metadata)?;
    validate_not_root(&file_metadata)?;
    validate_parent_exists_and_is_folder(config, source, &file_metadata)?;
    validate_path(config, source, &file_metadata)?;

    file_repo::insert_metadata(config, source, &file_metadata)
}

pub fn maybe_get_metadata(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<(FileMetadata, ClientFileMetadata)>, CoreError> {
    Ok(match metadata_repo::maybe_get(config, source, id)? {
        Some(metadata) => Some((
            metadata.clone(),
            client_conversion::generate_client_file_metadata(config, &metadata)?,
        )),
        None => None,
    })
}

pub fn get_metadata(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<(FileMetadata, ClientFileMetadata), CoreError> {
    maybe_get_metadata(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn rename(
    config: &Config,
    source: RepoSource,
    id: Uuid,
    new_name: &str,
) -> Result<(), CoreError> {
    account_repo::get(config)?;
    validate_file_name(new_name)?;

    let mut file_metadata = file_repo::get_metadata(config, source, id)?;
    file_metadata.name = file_encryption_service::create_name(&config, file_metadata.id, new_name)?;

    insert_metadata(config, source, &file_metadata)
}

pub fn move_(
    config: &Config,
    source: RepoSource,
    id: Uuid,
    new_parent: Uuid,
) -> Result<(), CoreError> {
    account_repo::get(config)?;

    let mut file_metadata = file_repo::get_metadata(config, source, id)?;
    let parent_metadata = validate_parent_exists_and_is_folder(config, source, &file_metadata)?;

    file_metadata.parent = new_parent;
    file_metadata.name =
        file_encryption_service::rekey_secret_filename(&config, &file_metadata, &parent_metadata)?;
    file_metadata.folder_access_keys = file_encryption_service::re_encrypt_key_for_file(
        &config,
        file_encryption_service::decrypt_key_for_file(&config, file_metadata.id)?,
        parent_metadata.id,
    )?;

    insert_metadata(config, source, &file_metadata)
}

pub fn delete(config: &Config, source: RepoSource, id: Uuid) -> Result<(), CoreError> {
    account_repo::get(config)?;

    let mut file_metadata = file_repo::get_metadata(config, source, id)?;

    file_metadata.deleted = true;

    insert_metadata(config, source, &file_metadata)
}

pub fn write_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
    content: &[u8],
) -> Result<(), CoreError> {
    account_repo::get(config)?;

    let file_metadata = file_repo::get_metadata(config, source, id)?;

    validate_is_document(&file_metadata)?;

    let digest = Sha256::digest(content);
    let compressed_content = file_compression_service::compress(content)?;
    let encrypted_content =
        file_encryption_service::write_to_document(&config, &compressed_content, &file_metadata)?;

    file_repo::insert_document(config, source, id, encrypted_content, &digest)
}

pub fn maybe_read_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<DecryptedDocument>, CoreError> {
    account_repo::get(config)?;

    if let Some(file_metadata) = file_repo::maybe_get_metadata(config, source, id)? {
        validate_is_document(&file_metadata)?;

        let encrypted_content = file_repo::get_document(config, source, id)?;
        let compressed_content =
            file_encryption_service::read_document(&config, &encrypted_content, &file_metadata)?;
        let content = file_compression_service::decompress(&compressed_content)?;

        Ok(Some(content))
    } else {
        Ok(None)
    }
}

pub fn read_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<DecryptedDocument, CoreError> {
    maybe_read_document(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn read_document_content(
    config: &Config,
    file_metadata: &FileMetadata,
    maybe_encrypted_content: &Option<EncryptedDocument>,
) -> Result<DecryptedDocument, CoreError> {
    account_repo::get(config)?;

    if let Some(encrypted_content) = maybe_encrypted_content {
        let compressed_content =
            file_encryption_service::read_document(&config, encrypted_content, &file_metadata)?;
        file_compression_service::decompress(&compressed_content)
    } else {
        Ok(Vec::new())
    }
}

pub fn save_document_to_disk(
    config: &Config,
    source: RepoSource,
    id: Uuid,
    location: String,
) -> Result<(), CoreError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&location))
        .map_err(CoreError::from)?
        .write_all(read_document(config, source, id)?.as_slice())
        .map_err(CoreError::from)?;
    Ok(())
}

pub fn get_all_document_ids(config: &Config, source: RepoSource) -> Result<Vec<Uuid>, CoreError> {
    account_repo::get(config)?;

    Ok(file_repo::get_all_metadata(&config, source)?
        .into_iter()
        .filter(|f| f.file_type == FileType::Document)
        .map(|f| f.id)
        .collect())
}

fn validate_not_root(file: &FileMetadata) -> Result<(), CoreError> {
    if file.id != file.parent {
        Ok(())
    } else {
        Err(CoreError::RootModificationInvalid)
    }
}

fn validate_is_document(file: &FileMetadata) -> Result<(), CoreError> {
    if file.file_type == FileType::Document {
        Ok(())
    } else {
        Err(CoreError::FileNotDocument)
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

fn validate_parent_exists_and_is_folder(
    config: &Config,
    source: RepoSource,
    file: &FileMetadata,
) -> Result<FileMetadata, CoreError> {
    let parent = file_repo::maybe_get_metadata(&config, source, file.parent)?
        .ok_or(CoreError::FileParentNonexistent)?;

    if parent.file_type == FileType::Folder {
        Ok(parent)
    } else {
        Err(CoreError::FileNotFolder)
    }
}

fn validate_path(
    config: &Config,
    source: RepoSource,
    file: &FileMetadata,
) -> Result<(), CoreError> {
    for child in file_repo::get_children(config, source, file.parent)? {
        if file_encryption_service::get_name(&config, &child)?
            == file_encryption_service::get_name(&config, &file)?
            && child.id != file.id
        {
            return Err(CoreError::PathTaken);
        }
    }
    Ok(())
}

fn validate_not_own_ancestor(
    config: &Config,
    source: RepoSource,
    file: &FileMetadata,
) -> Result<(), CoreError> {
    file_repo::get_with_ancestors(config, source, file.id)?;
    Ok(())
}
