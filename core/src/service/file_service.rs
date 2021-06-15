use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::model::state::Config;
use crate::repo::document_repo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::FindingParentsFailed;
use crate::repo::{account_repo, local_changes_repo};
use crate::service::file_compression_service;
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::{
    FileCreationError, KeyDecryptionFailure, UnableToGetKeyForUser,
};
use crate::service::file_service::DocumentRenameError::{CannotRenameRoot, FileDoesNotExist};
use crate::service::file_service::DocumentUpdateError::{
    AccessInfoCreationError, CouldNotFindFile, DbError, DocumentWriteError, FetchOldVersionError,
    FolderTreatedAsDocument,
};
use crate::service::file_service::FileMoveError::{
    CannotMoveRoot, FailedToDecryptKey, FailedToReEncryptKey, FileDoesNotExist as FileDNE,
    TargetParentDoesNotExist,
};
use crate::service::file_service::NewFileError::{
    DocumentTreatedAsFolder, FailedToWriteFileContent, FileEncryptionError, FileNameContainsSlash,
    FileNameEmpty, FileNameNotAvailable, MetadataRepoError,
};
use crate::service::file_service::ReadDocumentError::DocumentReadError;
use lockbook_crypto::clock_service;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::file_metadata::{FileMetadata, FileType};

#[derive(Debug)]
pub enum NewFileError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CouldNotFindParents,
    FileEncryptionError(file_encryption_service::FileCreationError),
    MetadataRepoError(file_metadata_repo::DbError),
    FailedToWriteFileContent(DocumentUpdateError),
    FailedToRecordChange(local_changes_repo::DbError),
    FileNameNotAvailable,
    DocumentTreatedAsFolder,
    FileNameEmpty,
    FileNameContainsSlash,
    NameDecryptionError(file_encryption_service::GetNameOfFileError),
}

pub fn create(
    config: &Config,
    name: &str,
    parent: Uuid,
    file_type: FileType,
) -> Result<FileMetadata, NewFileError> {
    if name.is_empty() {
        return Err(FileNameEmpty);
    }

    if name.contains('/') {
        return Err(FileNameContainsSlash);
    }

    let _account =
        account_repo::get_account(config).map_err(NewFileError::AccountRetrievalError)?;

    let parent = file_metadata_repo::maybe_get(&config, parent)
        .map_err(NewFileError::MetadataRepoError)?
        .ok_or(NewFileError::CouldNotFindParents)?;

    // Make sure parent is in fact a folder
    if parent.file_type == Document {
        return Err(DocumentTreatedAsFolder);
    }

    // Check that this file name is available
    for child in file_metadata_repo::get_children_non_recursively(config, parent.id)
        .map_err(MetadataRepoError)?
    {
        if file_encryption_service::get_name(&config, &child)
            .map_err(NewFileError::NameDecryptionError)?
            == name
        {
            return Err(FileNameNotAvailable);
        }
    }

    let new_metadata =
        file_encryption_service::create_file_metadata(&config, name, file_type, parent.id)
            .map_err(FileEncryptionError)?;

    file_metadata_repo::insert(config, &new_metadata).map_err(MetadataRepoError)?;
    local_changes_repo::track_new_file(config, new_metadata.id, clock_service::get_time)
        .map_err(NewFileError::FailedToRecordChange)?;

    if file_type == Document {
        write_document(config, new_metadata.id, &[]).map_err(FailedToWriteFileContent)?;
    }
    Ok(new_metadata)
}

#[derive(Debug)]
pub enum DocumentUpdateError {
    CouldNotFindFile,
    FolderTreatedAsDocument,
    FileEncryptionError(file_encryption_service::FileWriteError),
    FileCompressionError(std::io::Error),
    FileDecompressionError(std::io::Error),
    DocumentWriteError(document_repo::Error),
    FetchOldVersionError(document_repo::DbError),
    DecryptOldVersionError(file_encryption_service::UnableToReadFile),
    AccessInfoCreationError(UnableToGetKeyForUser),
    DbError(file_metadata_repo::DbError),
    FailedToRecordChange(local_changes_repo::DbError),
}

pub fn write_document(
    config: &Config,
    id: Uuid,
    content: &[u8],
) -> Result<(), DocumentUpdateError> {
    let file_metadata = file_metadata_repo::maybe_get(config, id)
        .map_err(DbError)?
        .ok_or(CouldNotFindFile)?;

    if file_metadata.file_type == Folder {
        return Err(FolderTreatedAsDocument);
    }

    let compressed_content = file_compression_service::compress(content)
        .map_err(DocumentUpdateError::FileCompressionError)?;

    let new_file =
        file_encryption_service::write_to_document(&config, &compressed_content, &file_metadata)
            .map_err(DocumentUpdateError::FileEncryptionError)?;

    file_metadata_repo::insert(config, &file_metadata).map_err(DbError)?;

    if let Some(old_encrypted) =
        document_repo::maybe_get(config, id).map_err(FetchOldVersionError)?
    {
        let decrypted =
            file_encryption_service::read_document(&config, &old_encrypted, &file_metadata)
                .map_err(DocumentUpdateError::DecryptOldVersionError)?;
        let decompressed = file_compression_service::decompress(&decrypted)
            .map_err(DocumentUpdateError::FileDecompressionError)?;

        let permanent_access_info = file_encryption_service::get_key_for_user(&config, id)
            .map_err(AccessInfoCreationError)?;

        local_changes_repo::track_edit(
            config,
            file_metadata.id,
            &old_encrypted,
            &permanent_access_info,
            Sha256::digest(&decompressed).to_vec(),
            Sha256::digest(&content).to_vec(),
            clock_service::get_time,
        )
        .map_err(DocumentUpdateError::FailedToRecordChange)?;
    };

    document_repo::insert(config, file_metadata.id, &new_file).map_err(DocumentWriteError)?;

    Ok(())
}

#[derive(Debug)]
pub enum DocumentRenameError {
    FileDoesNotExist,
    FileNameEmpty,
    FileNameContainsSlash,
    FileNameNotAvailable,
    CannotRenameRoot,
    DbError(file_metadata_repo::DbError),
    FailedToRecordChange(local_changes_repo::DbError),
    GetNameOfFileError(file_encryption_service::GetNameOfFileError),
    NameCreationError(file_encryption_service::CreateNameError),
}

pub fn rename_file(config: &Config, id: Uuid, new_name: &str) -> Result<(), DocumentRenameError> {
    if new_name.is_empty() {
        return Err(DocumentRenameError::FileNameEmpty);
    }

    if new_name.contains('/') {
        return Err(DocumentRenameError::FileNameContainsSlash);
    }

    match file_metadata_repo::maybe_get(config, id).map_err(DocumentRenameError::DbError)? {
        None => Err(FileDoesNotExist),
        Some(mut file) => {
            if file.id == file.parent {
                return Err(CannotRenameRoot);
            }

            let siblings = file_metadata_repo::get_children_non_recursively(config, file.parent)
                .map_err(DocumentRenameError::DbError)?;

            // Check that this file name is available
            for child in siblings {
                if file_encryption_service::get_name(&config, &child)
                    .map_err(DocumentRenameError::GetNameOfFileError)?
                    == new_name
                {
                    return Err(DocumentRenameError::FileNameNotAvailable);
                }
            }

            let old_file_name = file_encryption_service::get_name(&config, &file)
                .map_err(DocumentRenameError::GetNameOfFileError)?;

            local_changes_repo::track_rename(
                config,
                file.id,
                &old_file_name,
                new_name,
                clock_service::get_time,
            )
            .map_err(DocumentRenameError::FailedToRecordChange)?;

            file.name = file_encryption_service::create_name(&config, &file, new_name)
                .map_err(DocumentRenameError::NameCreationError)?;
            file_metadata_repo::insert(config, &file).map_err(DocumentRenameError::DbError)?;

            Ok(())
        }
    }
}

#[derive(Debug)]
pub enum FileMoveError {
    AccountRetrievalError(account_repo::AccountRepoError),
    TargetParentHasChildNamedThat,
    FolderMovedIntoItself,
    FileDoesNotExist,
    TargetParentDoesNotExist,
    DocumentTreatedAsFolder,
    CannotMoveRoot,
    FindingChildrenFailed(file_metadata_repo::FindingChildrenFailed),
    DbError(file_metadata_repo::DbError),
    FailedToRecordChange(local_changes_repo::DbError),
    FailedToDecryptKey(KeyDecryptionFailure),
    FailedToReEncryptKey(FileCreationError),
    CouldNotFindParents(FindingParentsFailed),
    ReKeyNameError(file_encryption_service::RekeySecretFilenameError),
}

pub fn move_file(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), FileMoveError> {
    let _account =
        account_repo::get_account(config).map_err(FileMoveError::AccountRetrievalError)?;

    let mut file = file_metadata_repo::maybe_get(config, id)
        .map_err(FileMoveError::DbError)?
        .ok_or(FileDNE)?;

    if file.id == file.parent {
        return Err(CannotMoveRoot);
    }

    let parent_metadata = file_metadata_repo::maybe_get(config, new_parent)
        .map_err(FileMoveError::DbError)?
        .ok_or(TargetParentDoesNotExist)?;

    if parent_metadata.file_type == Document {
        return Err(FileMoveError::DocumentTreatedAsFolder);
    }

    let siblings = file_metadata_repo::get_children_non_recursively(config, parent_metadata.id)
        .map_err(FileMoveError::DbError)?;

    let new_name = file_encryption_service::rekey_secret_filename(&config, &file, &parent_metadata)
        .map_err(FileMoveError::ReKeyNameError)?;

    // Check that this file name is available
    for child in siblings {
        if child.name == new_name {
            return Err(FileMoveError::TargetParentHasChildNamedThat);
        }
    }

    // Checking if a folder is being moved into itself or its children
    if file.file_type == FileType::Folder {
        let children = file_metadata_repo::get_and_get_children_recursively(config, id)
            .map_err(FileMoveError::FindingChildrenFailed)?;
        for child in children {
            if child.id == new_parent {
                return Err(FileMoveError::FolderMovedIntoItself);
            }
        }
    }

    let access_key = file_encryption_service::decrypt_key_for_file(&config, file.id)
        .map_err(FailedToDecryptKey)?;

    let new_access_info =
        file_encryption_service::re_encrypt_key_for_file(&config, access_key, parent_metadata.id)
            .map_err(FailedToReEncryptKey)?;

    local_changes_repo::track_move(
        config,
        file.id,
        file.parent,
        parent_metadata.id,
        clock_service::get_time,
    )
    .map_err(FileMoveError::FailedToRecordChange)?;

    file.parent = parent_metadata.id;
    file.folder_access_keys = new_access_info;
    file.name = new_name;

    file_metadata_repo::insert(config, &file).map_err(FileMoveError::DbError)?;
    Ok(())
}

#[derive(Debug)]
pub enum ReadDocumentError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CouldNotFindFile,
    DbError(file_metadata_repo::DbError),
    TreatedFolderAsDocument,
    DocumentReadError(document_repo::Error),
    CouldNotFindParents(FindingParentsFailed),
    FileEncryptionError(file_encryption_service::UnableToReadFile),
    FileDecompressionError(std::io::Error),
}

pub fn read_document(config: &Config, id: Uuid) -> Result<DecryptedDocument, ReadDocumentError> {
    let _account =
        account_repo::get_account(config).map_err(ReadDocumentError::AccountRetrievalError)?;

    let file_metadata = file_metadata_repo::maybe_get(config, id)
        .map_err(ReadDocumentError::DbError)?
        .ok_or(ReadDocumentError::CouldNotFindFile)?;

    if file_metadata.file_type == Folder {
        return Err(ReadDocumentError::TreatedFolderAsDocument);
    }

    let document = document_repo::get(config, id).map_err(DocumentReadError)?;

    let _parents = file_metadata_repo::get_with_all_parents(config, id)
        .map_err(ReadDocumentError::CouldNotFindParents)?;

    let compressed_content =
        file_encryption_service::read_document(&config, &document, &file_metadata)
            .map_err(ReadDocumentError::FileEncryptionError)?;

    let content = file_compression_service::decompress(&compressed_content)
        .map_err(ReadDocumentError::FileDecompressionError)?;

    Ok(content)
}

#[derive(Debug)]
pub enum DeleteDocumentError {
    CouldNotFindFile,
    FolderTreatedAsDocument,
    FailedToRecordChange(local_changes_repo::DbError),
    FailedToUpdateMetadata(file_metadata_repo::DbError),
    FailedToDeleteDocument(document_repo::Error),
    FailedToTrackDelete(local_changes_repo::DbError),
    DbError(file_metadata_repo::DbError),
}

pub fn delete_document(config: &Config, id: Uuid) -> Result<(), DeleteDocumentError> {
    let mut file_metadata = file_metadata_repo::maybe_get(config, id)
        .map_err(DeleteDocumentError::DbError)?
        .ok_or(DeleteDocumentError::CouldNotFindFile)?;

    if file_metadata.file_type == Folder {
        return Err(DeleteDocumentError::FolderTreatedAsDocument);
    }

    let new = if let Some(change) = local_changes_repo::get_local_changes(config, id)
        .map_err(DeleteDocumentError::FailedToTrackDelete)?
    {
        change.new
    } else {
        false
    };

    if !new {
        file_metadata.deleted = true;
        file_metadata_repo::insert(config, &file_metadata)
            .map_err(DeleteDocumentError::FailedToUpdateMetadata)?;
    } else {
        file_metadata_repo::non_recursive_delete(config, id)
            .map_err(DeleteDocumentError::FailedToUpdateMetadata)?;
    }

    document_repo::delete(config, id).map_err(DeleteDocumentError::FailedToDeleteDocument)?;
    local_changes_repo::track_delete(config, id, file_metadata.file_type, clock_service::get_time)
        .map_err(DeleteDocumentError::FailedToTrackDelete)?;

    Ok(())
}

#[derive(Debug)]
pub enum DeleteFolderError {
    MetadataError(file_metadata_repo::DbError),
    CouldNotFindFile,
    CannotDeleteRoot,
    FailedToDeleteMetadata(file_metadata_repo::DbError),
    FindingChildrenFailed(file_metadata_repo::FindingChildrenFailed),
    FailedToRecordChange(local_changes_repo::DbError),
    CouldNotFindParents(FindingParentsFailed),
    DocumentTreatedAsFolder,
    FailedToDeleteDocument(document_repo::Error),
    FailedToDeleteChangeEntry(local_changes_repo::DbError),
}

pub fn delete_folder(config: &Config, id: Uuid) -> Result<(), DeleteFolderError> {
    let file_metadata = file_metadata_repo::maybe_get(config, id)
        .map_err(DeleteFolderError::MetadataError)?
        .ok_or(DeleteFolderError::CouldNotFindFile)?;

    if file_metadata.id == file_metadata.parent {
        return Err(DeleteFolderError::CannotDeleteRoot);
    }

    if file_metadata.file_type == Document {
        return Err(DeleteFolderError::DocumentTreatedAsFolder);
    }

    local_changes_repo::track_delete(config, id, file_metadata.file_type, clock_service::get_time)
        .map_err(DeleteFolderError::FailedToRecordChange)?;

    let files_to_delete = file_metadata_repo::get_and_get_children_recursively(config, id)
        .map_err(DeleteFolderError::FindingChildrenFailed)?;

    // Server has told us we have the most recent version of all children in this directory and that we can delete now
    for mut file in files_to_delete {
        if file.file_type == Document {
            document_repo::delete(config, file.id)
                .map_err(DeleteFolderError::FailedToDeleteDocument)?;
        }

        let moved = if let Some(change) = local_changes_repo::get_local_changes(config, file.id)
            .map_err(DeleteFolderError::FailedToDeleteChangeEntry)?
        {
            change.moved.is_some()
        } else {
            false
        };

        if file.id != id && !moved {
            file_metadata_repo::non_recursive_delete(config, file.id)
                .map_err(DeleteFolderError::FailedToDeleteMetadata)?;

            local_changes_repo::delete(config, file.id)
                .map_err(DeleteFolderError::FailedToDeleteChangeEntry)?;
        } else {
            file.deleted = true;
            file_metadata_repo::insert(config, &file)
                .map_err(DeleteFolderError::FailedToDeleteMetadata)?;
        }
    }

    Ok(())
}
