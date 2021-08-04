use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::model::client_conversion;
use crate::model::client_conversion::ClientFileMetadata;
use crate::model::state::Config;
use crate::repo::document_repo;
use crate::repo::file_metadata_repo;
use crate::repo::{account_repo, local_changes_repo};
use crate::service::file_compression_service;
use crate::service::file_encryption_service;
use crate::service::path_service::{create_at_path, get_path_by_id};
use crate::CoreError;
use lockbook_crypto::clock_service;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::file_metadata::{FileMetadata, FileType};
use std::fs;
use std::fs::{DirEntry, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn create(
    config: &Config,
    name: &str,
    parent: Uuid,
    file_type: FileType,
) -> Result<FileMetadata, CoreError> {
    if name.is_empty() {
        return Err(CoreError::FileNameEmpty);
    }
    if name.contains('/') {
        return Err(CoreError::FileNameContainsSlash);
    }

    let _account = account_repo::get_account(config)?;

    let parent =
        file_metadata_repo::maybe_get(config, parent)?.ok_or(CoreError::FileParentNonexistent)?;

    // Make sure parent is in fact a folder
    if parent.file_type == Document {
        return Err(CoreError::FileNotFolder);
    }

    // Check that this file name is available
    for child in file_metadata_repo::get_children_non_recursively(config, parent.id)? {
        if file_encryption_service::get_name(config, &child)? == name {
            return Err(CoreError::PathTaken);
        }
    }

    let new_metadata =
        file_encryption_service::create_file_metadata(config, name, file_type, parent.id)?;

    file_metadata_repo::insert(config, &new_metadata)?;
    local_changes_repo::track_new_file(config, new_metadata.id, clock_service::get_time)?;

    if file_type == Document {
        write_document(config, new_metadata.id, &[])?;
    }
    Ok(new_metadata)
}

pub fn write_document(config: &Config, id: Uuid, content: &[u8]) -> Result<(), CoreError> {
    let _account = account_repo::get_account(config)?;

    let file_metadata =
        file_metadata_repo::maybe_get(config, id)?.ok_or(CoreError::FileNonexistent)?;

    if file_metadata.file_type == Folder {
        return Err(CoreError::FileNotDocument);
    }

    let compressed_content = file_compression_service::compress(content)?;
    let new_file =
        file_encryption_service::write_to_document(config, &compressed_content, &file_metadata)?;
    file_metadata_repo::insert(config, &file_metadata)?;

    if let Some(old_encrypted) = document_repo::maybe_get(config, id)? {
        let decrypted =
            file_encryption_service::read_document(config, &old_encrypted, &file_metadata)?;
        let decompressed = file_compression_service::decompress(&decrypted)?;
        let permanent_access_info = file_encryption_service::get_key_for_user(config, id)?;

        local_changes_repo::track_edit(
            config,
            file_metadata.id,
            &old_encrypted,
            &permanent_access_info,
            Sha256::digest(&decompressed).to_vec(),
            Sha256::digest(content).to_vec(),
            clock_service::get_time,
        )?;
    };

    document_repo::insert(config, file_metadata.id, &new_file)?;

    Ok(())
}

pub fn rename_file(config: &Config, id: Uuid, new_name: &str) -> Result<(), CoreError> {
    if new_name.is_empty() {
        return Err(CoreError::FileNameEmpty);
    }
    if new_name.contains('/') {
        return Err(CoreError::FileNameContainsSlash);
    }

    match file_metadata_repo::maybe_get(config, id)? {
        None => Err(CoreError::FileNonexistent),
        Some(mut file) => {
            if file.id == file.parent {
                return Err(CoreError::RootModificationInvalid);
            }

            let siblings = file_metadata_repo::get_children_non_recursively(config, file.parent)?;

            // Check that this file name is available
            for child in siblings {
                if file_encryption_service::get_name(config, &child)? == new_name {
                    return Err(CoreError::PathTaken);
                }
            }

            let old_file_name = file_encryption_service::get_name(config, &file)?;

            local_changes_repo::track_rename(
                config,
                file.id,
                &old_file_name,
                new_name,
                clock_service::get_time,
            )?;

            file.name = file_encryption_service::create_name(config, &file, new_name)?;
            file_metadata_repo::insert(config, &file)?;

            Ok(())
        }
    }
}

pub fn move_file(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), CoreError> {
    let _account = account_repo::get_account(config)?;

    let mut file = file_metadata_repo::maybe_get(config, id)?.ok_or(CoreError::FileNonexistent)?;
    if file.id == file.parent {
        return Err(CoreError::RootModificationInvalid);
    }

    let parent_metadata = file_metadata_repo::maybe_get(config, new_parent)?
        .ok_or(CoreError::FileParentNonexistent)?;
    if parent_metadata.file_type == Document {
        return Err(CoreError::FileNotFolder);
    }

    let siblings = file_metadata_repo::get_children_non_recursively(config, parent_metadata.id)?;
    let new_name = file_encryption_service::rekey_secret_filename(config, &file, &parent_metadata)?;

    // Check that this file name is available
    for child in siblings {
        if child.name == new_name {
            return Err(CoreError::PathTaken);
        }
    }

    // Checking if a folder is being moved into itself or its children
    if file.file_type == FileType::Folder {
        let children = file_metadata_repo::get_and_get_children_recursively(config, id)?;
        for child in children {
            if child.id == new_parent {
                return Err(CoreError::FolderMovedIntoSelf);
            }
        }
    }

    let access_key = file_encryption_service::decrypt_key_for_file(config, file.id)?;
    let new_access_info =
        file_encryption_service::re_encrypt_key_for_file(config, access_key, parent_metadata.id)?;

    local_changes_repo::track_move(
        config,
        file.id,
        file.parent,
        parent_metadata.id,
        clock_service::get_time,
    )?;

    file.parent = parent_metadata.id;
    file.folder_access_keys = new_access_info;
    file.name = new_name;

    file_metadata_repo::insert(config, &file)?;
    Ok(())
}

pub fn read_document(config: &Config, id: Uuid) -> Result<DecryptedDocument, CoreError> {
    let _account = account_repo::get_account(config)?;

    let file_metadata =
        file_metadata_repo::maybe_get(config, id)?.ok_or(CoreError::FileNonexistent)?;

    if file_metadata.file_type == Folder {
        return Err(CoreError::FileNotDocument);
    }

    let document = document_repo::get(config, id)?;
    let compressed_content =
        file_encryption_service::read_document(config, &document, &file_metadata)?;
    let content = file_compression_service::decompress(&compressed_content)?;

    Ok(content)
}

pub fn save_document_to_disk(config: &Config, id: Uuid, location: String) -> Result<(), CoreError> {
    let document_content = read_document(config, id)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&location))
        .map_err(CoreError::from)?;

    file.write_all(document_content.as_slice())
        .map_err(CoreError::from)
}

pub fn delete_document(config: &Config, id: Uuid) -> Result<(), CoreError> {
    let mut file_metadata =
        file_metadata_repo::maybe_get(config, id)?.ok_or(CoreError::FileNonexistent)?;

    if file_metadata.file_type == Folder {
        return Err(CoreError::FileNotDocument);
    }

    let new = if let Some(change) = local_changes_repo::get_local_changes(config, id)? {
        change.new
    } else {
        false
    };

    if !new {
        file_metadata.deleted = true;
        file_metadata_repo::insert(config, &file_metadata)?;
    } else {
        file_metadata_repo::non_recursive_delete(config, id)?;
    }

    document_repo::delete(config, id)?;
    local_changes_repo::track_delete(config, id, file_metadata.file_type, clock_service::get_time)?;

    Ok(())
}

pub fn delete_folder(config: &Config, id: Uuid) -> Result<(), CoreError> {
    let file_metadata =
        file_metadata_repo::maybe_get(config, id)?.ok_or(CoreError::FileNonexistent)?;

    if file_metadata.id == file_metadata.parent {
        return Err(CoreError::RootModificationInvalid);
    }
    if file_metadata.file_type == Document {
        return Err(CoreError::FileNotFolder);
    }

    local_changes_repo::track_delete(config, id, file_metadata.file_type, clock_service::get_time)?;

    let files_to_delete = file_metadata_repo::get_and_get_children_recursively(config, id)?;

    for mut file in files_to_delete {
        if file.file_type == Document {
            document_repo::delete(config, file.id)?;
        }

        let moved = if let Some(change) = local_changes_repo::get_local_changes(config, file.id)? {
            change.moved.is_some()
        } else {
            false
        };

        if file.id != id && !moved {
            file_metadata_repo::non_recursive_delete(config, file.id)?;

            local_changes_repo::delete(config, file.id)?;
        } else {
            file.deleted = true;
            file_metadata_repo::insert(config, &file)?;
        }
    }

    Ok(())
}

pub fn import_file(config: &Config, parent: Uuid, location: String) -> Result<(), CoreError> {
    let disk_path = Path::new(&location);

    import_file_recursively(
        &config,
        &disk_path,
        get_path_by_id(config, parent)?.as_str(),
    )
}

fn import_file_recursively(
    config: &Config,
    disk_path: &Path,
    lockbook_path: &str,
) -> Result<(), CoreError> {
    let lockbook_path_with_new = format!(
        "{}{}",
        lockbook_path,
        disk_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(CoreError::DiskPathInvalid)?
    );

    if disk_path.is_file() {
        let content = fs::read(&disk_path).map_err(CoreError::from)?;
        let file_metadata = create_at_path(config, lockbook_path_with_new.as_str())?;

        write_document(config, file_metadata.id, content.as_slice())?;
    } else {
        let children: Vec<Result<DirEntry, std::io::Error>> =
            fs::read_dir(disk_path).map_err(CoreError::from)?.collect();

        if children.is_empty() {
            create_at_path(config, &lockbook_path_with_new)?;
        } else {
            for maybe_child in children {
                let child_path = maybe_child.map_err(CoreError::from)?.path();

                import_file_recursively(config, &child_path, &lockbook_path_with_new)?;
            }
        }
    }

    Ok(())
}

pub fn export_file(config: &Config, parent: Uuid, location: String) -> Result<(), CoreError> {
    let dest = Path::new(&location).to_path_buf();

    if dest.is_file() {
        return Err(CoreError::DiskPathInvalid);
    }

    let file_metadata = client_conversion::generate_client_file_metadata(
        config,
        &file_metadata_repo::get(config, parent)?,
    )?;
    export_file_recursively(config, &file_metadata, &dest)
}

fn export_file_recursively(
    config: &Config,
    parent_file_metadata: &ClientFileMetadata,
    dest: &PathBuf,
) -> Result<(), CoreError> {
    let dest_with_new = dest.join(&parent_file_metadata.name);

    match parent_file_metadata.file_type {
        FileType::Folder => {
            println!("FOLDER");
            let children =
                file_metadata_repo::get_children_non_recursively(config, parent_file_metadata.id)?;
            fs::create_dir(dest_with_new.clone()).map_err(CoreError::from)?;

            for child in children.iter() {
                let child_file_metadata =
                    client_conversion::generate_client_file_metadata(config, &child)?;

                export_file_recursively(config, &child_file_metadata, &dest_with_new)?;
            }
        }
        FileType::Document => {
            println!("DOCUMENT");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dest_with_new)
                .map_err(CoreError::from)?;

            file.write_all(read_document(config, parent_file_metadata.id)?.as_slice())
                .map_err(CoreError::from)?;
        }
    }

    Ok(())
}
