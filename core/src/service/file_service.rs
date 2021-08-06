use crate::model::client_conversion::{self, ClientFileMetadata};
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::{account_repo, file_repo, metadata_repo};
use crate::service::file_compression_service;
use crate::service::file_encryption_service;
use crate::CoreError;
use lockbook_models::account::Account;
use lockbook_models::crypto::{DecryptedDocument, EncryptedDocument};
use lockbook_models::file_metadata::{FileMetadata, FileType};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

// return client file metadata? what are the usages of the functions here and is client file metadata ok for those
// no, the usages are:
// - sending requests to the server (that's all)

pub fn create_root(
    config: &Config,
    account: &Account,
    source: RepoSource,
) -> Result<FileMetadata, CoreError> {
    let file_metadata = file_encryption_service::create_metadata_for_root_folder(&account)?;

    validate_is_root(&file_metadata)?;

    file_repo::insert_metadata(config, source, &file_metadata)?;
    Ok(file_metadata)
}

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

fn validate_is_root(file: &FileMetadata) -> Result<(), CoreError> {
    if file.id == file.parent {
        Ok(())
    } else {
        Err(CoreError::RootModificationInvalid)
    }
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
