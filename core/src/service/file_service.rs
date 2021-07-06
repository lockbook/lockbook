use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::{account_repo, file_repo};
use crate::service::file_compression_service;
use crate::service::file_encryption_service;
use crate::CoreError;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::{FileMetadata, FileType};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

pub fn create(
    config: &Config,
    name: &str,
    parent: Uuid,
    file_type: FileType,
) -> Result<FileMetadata, CoreError> {
    validate_file_name(name)?;
    account_repo::get(config)?;

    let file_metadata =
        file_encryption_service::create_file_metadata(&config, name, file_type, parent)?;

    validate_path(config, &file_metadata)?;
    validate_parent_exists_and_is_folder(config, &file_metadata)?;

    file_repo::insert_metadata(config, RepoSource::Local, &file_metadata)?;
    Ok(file_metadata)
}

pub fn rename(config: &Config, id: Uuid, new_name: &str) -> Result<(), CoreError> {
    account_repo::get(config)?;

    let mut file_metadata = file_repo::get_metadata(config, id)?.0;
    file_metadata.name = file_encryption_service::create_name(&config, &file_metadata, new_name)?;

    validate_file_name(new_name)?;
    validate_not_root(&file_metadata)?;
    validate_path(config, &file_metadata)?;

    file_repo::insert_metadata(config, RepoSource::Local, &file_metadata)
}

pub fn move_(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), CoreError> {
    account_repo::get(config)?;

    let mut file_metadata = file_repo::get_metadata(config, id)?.0;
    let parent_metadata = validate_parent_exists_and_is_folder(config, &file_metadata)?;

    file_metadata.parent = new_parent;
    file_metadata.name =
        file_encryption_service::rekey_secret_filename(&config, &file_metadata, &parent_metadata)?;
    file_metadata.folder_access_keys = file_encryption_service::re_encrypt_key_for_file(
        &config,
        file_encryption_service::decrypt_key_for_file(&config, file_metadata.id)?,
        parent_metadata.id,
    )?;

    validate_not_root(&file_metadata)?;
    validate_path(config, &file_metadata)?;
    validate_not_own_ancestor(config, &file_metadata)?;

    file_repo::insert_metadata(config, RepoSource::Local, &file_metadata)
}

pub fn delete(config: &Config, id: Uuid) -> Result<(), CoreError> {
    account_repo::get(config)?;

    let mut file_metadata = file_repo::get_metadata(config, id)?.0;

    file_metadata.deleted = true;

    validate_not_root(&file_metadata)?;

    file_repo::insert_metadata(config, RepoSource::Local, &file_metadata)
}

pub fn write_document(config: &Config, id: Uuid, content: &[u8]) -> Result<(), CoreError> {
    account_repo::get(config)?;

    let file_metadata = file_repo::get_metadata(config, id)?.0;
    validate_is_document(&file_metadata)?;

    let digest = Sha256::digest(content);
    let compressed_content = file_compression_service::compress(content)?;
    let encrypted_content =
        file_encryption_service::write_to_document(&config, &compressed_content, &file_metadata)?;
    file_repo::insert_document(config, RepoSource::Local, id, encrypted_content, &digest)
}

pub fn read_document(config: &Config, id: Uuid) -> Result<DecryptedDocument, CoreError> {
    account_repo::get(config)?;

    let file_metadata = file_repo::get_metadata(config, id)?.0;
    validate_is_document(&file_metadata)?;

    let encrypted_content = file_repo::get_document(config, id)?.0;
    let compressed_content =
        file_encryption_service::read_document(&config, &encrypted_content, &file_metadata)?;
    let content = file_compression_service::decompress(&compressed_content)?;

    Ok(content)
}

pub fn save_document_to_disk(config: &Config, id: Uuid, location: String) -> Result<(), CoreError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&location))
        .map_err(CoreError::from)?
        .write_all(read_document(config, id)?.as_slice())
        .map_err(CoreError::from)?;
    Ok(())
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
    file: &FileMetadata,
) -> Result<FileMetadata, CoreError> {
    let parent = file_repo::maybe_get_metadata(&config, file.parent)?
        .ok_or(CoreError::FileParentNonexistent)?
        .0;

    if parent.file_type == FileType::Folder {
        Ok(parent)
    } else {
        Err(CoreError::FileNotFolder)
    }
}

fn validate_path(config: &Config, file: &FileMetadata) -> Result<(), CoreError> {
    for child in file_repo::get_children(config, file.parent)? {
        if file_encryption_service::get_name(&config, &child)?
            == file_encryption_service::get_name(&config, &file)?
            && child.id != file.id
        {
            return Err(CoreError::PathTaken);
        }
    }
    Ok(())
}

fn validate_not_own_ancestor(config: &Config, file: &FileMetadata) -> Result<(), CoreError> {
    file_repo::get_with_ancestors(config, file.id)?;
    Ok(())
}
