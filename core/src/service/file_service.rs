use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::model::state::Config;
use crate::repo::document_repo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::FindingParentsFailed;
use crate::repo::local_changes_repo::LocalChangesRepo;
use crate::repo::{account_repo, local_changes_repo};
use crate::service::file_compression_service::FileCompressionService;
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::{
    FileCreationError, FileEncryptionService, KeyDecryptionFailure, UnableToGetKeyForUser,
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
    DocumentTreatedAsFolder, FailedToWriteFileContent, FileCryptoError, FileNameContainsSlash,
    FileNameEmpty, FileNameNotAvailable, MetadataRepoError,
};
use crate::service::file_service::NewFileFromPathError::{
    FailedToCreateChild, FileAlreadyExists, NoRoot, PathContainsEmptyFile, PathDoesntStartWithRoot,
};
use crate::service::file_service::ReadDocumentError::DocumentReadError;
use lockbook_crypto::clock_service;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::file_metadata::{FileMetadata, FileType};

#[derive(Debug)]
pub enum NewFileError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CouldNotFindParents(FindingParentsFailed),
    FileCryptoError(file_encryption_service::FileCreationError),
    MetadataRepoError(file_metadata_repo::DbError),
    FailedToWriteFileContent(DocumentUpdateError),
    FailedToRecordChange(local_changes_repo::DbError),
    FileNameNotAvailable,
    DocumentTreatedAsFolder,
    FileNameEmpty,
    FileNameContainsSlash,
}

#[derive(Debug)]
pub enum NewFileFromPathError {
    DbError(file_metadata_repo::DbError),
    NoRoot,
    PathDoesntStartWithRoot,
    PathContainsEmptyFile,
    FailedToCreateChild(NewFileError),
    FailedToRecordChange(local_changes_repo::DbError),
    FileAlreadyExists,
}

#[derive(Debug)]
pub enum DocumentUpdateError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CouldNotFindFile,
    CouldNotFindParents(FindingParentsFailed),
    FolderTreatedAsDocument,
    FileCryptoError(file_encryption_service::FileWriteError),
    FileCompressionError(std::io::Error),
    FileDecompressionError(std::io::Error),
    DocumentWriteError(document_repo::Error),
    FetchOldVersionError(document_repo::DbError),
    DecryptOldVersionError(file_encryption_service::UnableToReadFile),
    AccessInfoCreationError(UnableToGetKeyForUser),
    DbError(file_metadata_repo::DbError),
    FailedToRecordChange(local_changes_repo::DbError),
}

#[derive(Debug)]
pub enum ReadDocumentError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CouldNotFindFile,
    DbError(file_metadata_repo::DbError),
    TreatedFolderAsDocument,
    DocumentReadError(document_repo::Error),
    CouldNotFindParents(FindingParentsFailed),
    FileCryptoError(file_encryption_service::UnableToReadFile),
    FileDecompressionError(std::io::Error),
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

pub trait FileService {
    fn create(
        config: &Config,
        name: &str,
        parent: Uuid,
        file_type: FileType,
    ) -> Result<FileMetadata, NewFileError>;

    fn create_at_path(
        config: &Config,
        path_and_name: &str,
    ) -> Result<FileMetadata, NewFileFromPathError>;

    fn write_document(config: &Config, id: Uuid, content: &[u8])
        -> Result<(), DocumentUpdateError>;

    fn rename_file(config: &Config, id: Uuid, new_name: &str) -> Result<(), DocumentRenameError>;

    fn move_file(
        config: &Config,
        file_metadata: Uuid,
        new_parent: Uuid,
    ) -> Result<(), FileMoveError>;

    fn read_document(config: &Config, id: Uuid) -> Result<DecryptedDocument, ReadDocumentError>;

    fn delete_document(config: &Config, id: Uuid) -> Result<(), DeleteDocumentError>;

    fn delete_folder(config: &Config, id: Uuid) -> Result<(), DeleteFolderError>;
}

pub struct FileServiceImpl<
    FileDb: DocumentRepo,
    ChangesDb: LocalChangesRepo,
    FileCrypto: FileEncryptionService,
    FileCompression: FileCompressionService,
> {
    _files: FileDb,
    _changes_db: ChangesDb,
    _file_crypto: FileCrypto,
    _file_compression: FileCompression,
}

impl<
        FileDb: DocumentRepo,
        ChangesDb: LocalChangesRepo,
        FileCrypto: FileEncryptionService,
        FileCompression: FileCompressionService,
    > FileService for FileServiceImpl<FileDb, ChangesDb, FileCrypto, FileCompression>
{
    fn create(
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

        let account =
            account_repo::get_account(config).map_err(NewFileError::AccountRetrievalError)?;

        let parents = file_metadata_repo::get_with_all_parents(config, parent)
            .map_err(NewFileError::CouldNotFindParents)?;

        // Make sure parent is in fact a folder
        if let Some(parent) = parents.get(&parent) {
            if parent.file_type == Document {
                return Err(DocumentTreatedAsFolder);
            }
        }

        // Check that this file name is available
        for child in file_metadata_repo::get_children_non_recursively(config, parent)
            .map_err(MetadataRepoError)?
        {
            if child.name == name {
                return Err(FileNameNotAvailable);
            }
        }

        let new_metadata =
            FileCrypto::create_file_metadata(name, file_type, parent, &account, parents)
                .map_err(FileCryptoError)?;

        file_metadata_repo::insert(config, &new_metadata).map_err(MetadataRepoError)?;
        ChangesDb::track_new_file(config, new_metadata.id, clock_service::get_time)
            .map_err(NewFileError::FailedToRecordChange)?;

        if file_type == Document {
            Self::write_document(config, new_metadata.id, &[]).map_err(FailedToWriteFileContent)?;
        }
        Ok(new_metadata)
    }

    fn create_at_path(
        config: &Config,
        path_and_name: &str,
    ) -> Result<FileMetadata, NewFileFromPathError> {
        if path_and_name.contains("//") {
            return Err(PathContainsEmptyFile);
        }

        debug!("Creating path at: {}", path_and_name);
        let path_components: Vec<&str> = path_and_name
            .split('/')
            .collect::<Vec<&str>>()
            .into_iter()
            .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
            .collect::<Vec<&str>>();

        let is_folder = path_and_name.ends_with('/');
        debug!("is folder: {}", is_folder);

        let mut current = file_metadata_repo::get_root(config)
            .map_err(NewFileFromPathError::DbError)?
            .ok_or(NoRoot)?;

        if current.name != path_components[0] {
            return Err(PathDoesntStartWithRoot);
        }

        if path_components.len() == 1 {
            return Err(FileAlreadyExists);
        }

        // We're going to look ahead, and find or create the right child
        'path: for index in 0..path_components.len() - 1 {
            let children = file_metadata_repo::get_children_non_recursively(config, current.id)
                .map_err(NewFileFromPathError::DbError)?;
            debug!(
                "children: {:?}",
                children
                    .clone()
                    .into_iter()
                    .map(|f| f.name)
                    .collect::<Vec<String>>()
            );

            let next_name = path_components[index + 1];
            debug!("child we're searching for: {}", next_name);

            for child in children {
                if child.name == next_name {
                    // If we're at the end and we find this child, that means this path already exists
                    if index == path_components.len() - 2 {
                        return Err(FileAlreadyExists);
                    }

                    if child.file_type == Folder {
                        current = child;
                        continue 'path; // Child exists, onto the next one
                    }
                }
            }
            debug!("child not found!");

            // Child does not exist, create it
            let file_type = if is_folder || index != path_components.len() - 2 {
                Folder
            } else {
                Document
            };

            current = Self::create(config, next_name, current.id, file_type)
                .map_err(FailedToCreateChild)?;
        }

        Ok(current)
    }

    fn write_document(
        config: &Config,
        id: Uuid,
        content: &[u8],
    ) -> Result<(), DocumentUpdateError> {
        let account = account_repo::get_account(config)
            .map_err(DocumentUpdateError::AccountRetrievalError)?;

        let file_metadata = file_metadata_repo::maybe_get(config, id)
            .map_err(DbError)?
            .ok_or(CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(FolderTreatedAsDocument);
        }

        let parents = file_metadata_repo::get_with_all_parents(config, id)
            .map_err(DocumentUpdateError::CouldNotFindParents)?;

        let compressed_content = FileCompression::compress(content)
            .map_err(DocumentUpdateError::FileCompressionError)?;

        let new_file = FileCrypto::write_to_document(
            &account,
            &compressed_content,
            &file_metadata,
            parents.clone(),
        )
        .map_err(DocumentUpdateError::FileCryptoError)?;

        file_metadata_repo::insert(config, &file_metadata).map_err(DbError)?;

        if let Some(old_encrypted) = FileDb::maybe_get(config, id).map_err(FetchOldVersionError)? {
            let decrypted = FileCrypto::read_document(
                &account,
                &old_encrypted,
                &file_metadata,
                parents.clone(),
            )
            .map_err(DocumentUpdateError::DecryptOldVersionError)?;
            let decompressed = FileCompression::decompress(&decrypted)
                .map_err(DocumentUpdateError::FileDecompressionError)?;

            let permanent_access_info = FileCrypto::get_key_for_user(&account, id, parents)
                .map_err(AccessInfoCreationError)?;

            ChangesDb::track_edit(
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

        FileDb::insert(config, file_metadata.id, &new_file).map_err(DocumentWriteError)?;

        Ok(())
    }

    fn rename_file(config: &Config, id: Uuid, new_name: &str) -> Result<(), DocumentRenameError> {
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

                let siblings =
                    file_metadata_repo::get_children_non_recursively(config, file.parent)
                        .map_err(DocumentRenameError::DbError)?;

                // Check that this file name is available
                for child in siblings {
                    if child.name == new_name {
                        return Err(DocumentRenameError::FileNameNotAvailable);
                    }
                }

                ChangesDb::track_rename(
                    config,
                    file.id,
                    &file.name,
                    new_name,
                    clock_service::get_time,
                )
                .map_err(DocumentRenameError::FailedToRecordChange)?;

                file.name = new_name.to_string();
                file_metadata_repo::insert(config, &file).map_err(DocumentRenameError::DbError)?;

                Ok(())
            }
        }
    }

    fn move_file(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), FileMoveError> {
        let account =
            account_repo::get_account(config).map_err(FileMoveError::AccountRetrievalError)?;

        match file_metadata_repo::maybe_get(config, id).map_err(FileMoveError::DbError)? {
            None => Err(FileDNE),
            Some(mut file) => {
                if file.id == file.parent {
                    return Err(CannotMoveRoot);
                }

                match file_metadata_repo::maybe_get(config, new_parent)
                    .map_err(FileMoveError::DbError)?
                {
                    None => Err(TargetParentDoesNotExist),
                    Some(parent_metadata) => {
                        if parent_metadata.file_type == Document {
                            return Err(FileMoveError::DocumentTreatedAsFolder);
                        }

                        let siblings = file_metadata_repo::get_children_non_recursively(
                            config,
                            parent_metadata.id,
                        )
                        .map_err(FileMoveError::DbError)?;

                        // Check that this file name is available
                        for child in siblings {
                            if child.name == file.name {
                                return Err(FileMoveError::TargetParentHasChildNamedThat);
                            }
                        }

                        // Checking if a folder is being moved into itself or its children
                        if file.file_type == FileType::Folder {
                            let children =
                                file_metadata_repo::get_and_get_children_recursively(config, id)
                                    .map_err(FileMoveError::FindingChildrenFailed)?;
                            for child in children {
                                if child.id == new_parent {
                                    return Err(FileMoveError::FolderMovedIntoItself);
                                }
                            }
                        }

                        // Good to move
                        let old_parents = file_metadata_repo::get_with_all_parents(config, file.id)
                            .map_err(FileMoveError::CouldNotFindParents)?;

                        let access_key =
                            FileCrypto::decrypt_key_for_file(&account, file.id, old_parents)
                                .map_err(FailedToDecryptKey)?;

                        let new_parents =
                            file_metadata_repo::get_with_all_parents(config, parent_metadata.id)
                                .map_err(FileMoveError::CouldNotFindParents)?;

                        let new_access_info = FileCrypto::re_encrypt_key_for_file(
                            &account,
                            access_key,
                            parent_metadata.id,
                            new_parents,
                        )
                        .map_err(FailedToReEncryptKey)?;

                        ChangesDb::track_move(
                            config,
                            file.id,
                            file.parent,
                            parent_metadata.id,
                            clock_service::get_time,
                        )
                        .map_err(FileMoveError::FailedToRecordChange)?;
                        file.parent = parent_metadata.id;
                        file.folder_access_keys = new_access_info;

                        file_metadata_repo::insert(config, &file)
                            .map_err(FileMoveError::DbError)?;
                        Ok(())
                    }
                }
            }
        }
    }

    fn read_document(config: &Config, id: Uuid) -> Result<DecryptedDocument, ReadDocumentError> {
        let account =
            account_repo::get_account(config).map_err(ReadDocumentError::AccountRetrievalError)?;

        let file_metadata = file_metadata_repo::maybe_get(config, id)
            .map_err(ReadDocumentError::DbError)?
            .ok_or(ReadDocumentError::CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(ReadDocumentError::TreatedFolderAsDocument);
        }

        let document = FileDb::get(config, id).map_err(DocumentReadError)?;

        let parents = file_metadata_repo::get_with_all_parents(config, id)
            .map_err(ReadDocumentError::CouldNotFindParents)?;

        let compressed_content =
            FileCrypto::read_document(&account, &document, &file_metadata, parents)
                .map_err(ReadDocumentError::FileCryptoError)?;

        let content = FileCompression::decompress(&compressed_content)
            .map_err(ReadDocumentError::FileDecompressionError)?;

        Ok(content)
    }

    fn delete_document(config: &Config, id: Uuid) -> Result<(), DeleteDocumentError> {
        let mut file_metadata = file_metadata_repo::maybe_get(config, id)
            .map_err(DeleteDocumentError::DbError)?
            .ok_or(DeleteDocumentError::CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(DeleteDocumentError::FolderTreatedAsDocument);
        }

        let new = if let Some(change) = ChangesDb::get_local_changes(config, id)
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

        FileDb::delete(config, id).map_err(DeleteDocumentError::FailedToDeleteDocument)?;
        ChangesDb::track_delete(config, id, file_metadata.file_type, clock_service::get_time)
            .map_err(DeleteDocumentError::FailedToTrackDelete)?;

        Ok(())
    }

    fn delete_folder(config: &Config, id: Uuid) -> Result<(), DeleteFolderError> {
        let file_metadata = file_metadata_repo::maybe_get(config, id)
            .map_err(DeleteFolderError::MetadataError)?
            .ok_or(DeleteFolderError::CouldNotFindFile)?;

        if file_metadata.id == file_metadata.parent {
            return Err(DeleteFolderError::CannotDeleteRoot);
        }

        if file_metadata.file_type == Document {
            return Err(DeleteFolderError::DocumentTreatedAsFolder);
        }

        ChangesDb::track_delete(config, id, file_metadata.file_type, clock_service::get_time)
            .map_err(DeleteFolderError::FailedToRecordChange)?;

        let files_to_delete = file_metadata_repo::get_and_get_children_recursively(config, id)
            .map_err(DeleteFolderError::FindingChildrenFailed)?;

        // Server has told us we have the most recent version of all children in this directory and that we can delete now
        for mut file in files_to_delete {
            if file.file_type == Document {
                FileDb::delete(config, file.id)
                    .map_err(DeleteFolderError::FailedToDeleteDocument)?;
            }

            let moved = if let Some(change) = ChangesDb::get_local_changes(config, file.id)
                .map_err(DeleteFolderError::FailedToDeleteChangeEntry)?
            {
                change.moved.is_some()
            } else {
                false
            };

            if file.id != id && !moved {
                file_metadata_repo::non_recursive_delete(config, file.id)
                    .map_err(DeleteFolderError::FailedToDeleteMetadata)?;

                ChangesDb::delete(config, file.id)
                    .map_err(DeleteFolderError::FailedToDeleteChangeEntry)?;
            } else {
                file.deleted = true;
                file_metadata_repo::insert(config, &file)
                    .map_err(DeleteFolderError::FailedToDeleteMetadata)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::state::temp_config;
    use crate::repo::account_repo;
    use crate::repo::document_repo::DocumentRepo;
    use crate::repo::file_metadata_repo;
    use crate::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
    use crate::repo::local_changes_repo::LocalChangesRepo;
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::service::file_service::{
        DeleteFolderError, DocumentRenameError, FileMoveError, FileService, NewFileError,
    };
    use crate::{
        init_logger, DefaultDocumentRepo, DefaultFileEncryptionService, DefaultFileService,
        DefaultLocalChangesRepo, NewFileFromPathError,
    };
    use libsecp256k1::SecretKey;
    use lockbook_models::account::Account;
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use rand::rngs::OsRng;

    macro_rules! assert_no_metadata_problems (
        ($db:expr) => {
            assert!(file_metadata_repo::test_repo_integrity($db)
                .unwrap()
                .is_empty());
        }
    );

    macro_rules! assert_total_local_changes (
        ($db:expr, $total:literal) => {
            assert_eq!(
                DefaultLocalChangesRepo::get_all_local_changes($db)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    macro_rules! assert_total_filtered_paths (
        ($db:expr, $filter:expr, $total:literal) => {
            assert_eq!(
                file_metadata_repo::get_all_paths($db, $filter)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    fn test_account() -> Account {
        Account {
            username: String::from("username"),
            api_url: "ftp://uranus.net".to_string(),
            private_key: SecretKey::random(&mut OsRng),
        }
    }

    #[test]
    fn file_service_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        assert!(file_metadata_repo::get_root(config).unwrap().is_none());

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert!(file_metadata_repo::get_root(config).unwrap().is_some());
        assert_no_metadata_problems!(config);

        assert!(matches!(
            DefaultFileService::create(config, "", root.id, Document).unwrap_err(),
            NewFileError::FileNameEmpty
        ));

        let folder1 = DefaultFileService::create(config, "TestFolder1", root.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder2 =
            DefaultFileService::create(config, "TestFolder2", folder1.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder3 =
            DefaultFileService::create(config, "TestFolder3", folder2.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder4 =
            DefaultFileService::create(config, "TestFolder4", folder3.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder5 =
            DefaultFileService::create(config, "TestFolder5", folder4.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let file = DefaultFileService::create(config, "test.text", folder5.id, Document).unwrap();
        assert_no_metadata_problems!(config);

        assert_total_filtered_paths!(config, Some(FoldersOnly), 6);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 1);
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 1);

        DefaultFileService::write_document(config, file.id, "5 folders deep".as_bytes()).unwrap();

        assert_eq!(
            DefaultFileService::read_document(config, file.id).unwrap(),
            "5 folders deep".as_bytes()
        );
        assert!(DefaultFileService::read_document(config, folder4.id).is_err());
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn path_calculations_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        assert_total_filtered_paths!(config, None, 0);

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_total_filtered_paths!(config, None, 1);
        assert_eq!(
            file_metadata_repo::get_all_paths(config, None)
                .unwrap()
                .get(0)
                .unwrap(),
            "username/"
        );

        assert_no_metadata_problems!(config);

        let folder1 = DefaultFileService::create(config, "TestFolder1", root.id, Folder).unwrap();
        assert_total_filtered_paths!(config, None, 2);
        assert!(file_metadata_repo::get_all_paths(config, None)
            .unwrap()
            .contains(&"username/".to_string()));
        assert!(file_metadata_repo::get_all_paths(config, None)
            .unwrap()
            .contains(&"username/TestFolder1/".to_string()));

        assert_no_metadata_problems!(config);

        let folder2 =
            DefaultFileService::create(config, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 =
            DefaultFileService::create(config, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 =
            DefaultFileService::create(config, "TestFolder4", folder3.id, Folder).unwrap();

        DefaultFileService::create(config, "TestFolder5", folder4.id, Folder).unwrap();
        DefaultFileService::create(config, "test1.text", folder4.id, Document).unwrap();
        DefaultFileService::create(config, "test2.text", folder2.id, Document).unwrap();
        DefaultFileService::create(config, "test3.text", folder2.id, Document).unwrap();
        DefaultFileService::create(config, "test4.text", folder2.id, Document).unwrap();
        DefaultFileService::create(config, "test5.text", folder2.id, Document).unwrap();
        assert_no_metadata_problems!(config);

        assert!(file_metadata_repo::get_all_paths(config, None)
            .unwrap()
            .contains(&"username/TestFolder1/TestFolder2/test3.text".to_string()));
        assert!(file_metadata_repo::get_all_paths(config, None)
            .unwrap()
            .contains(
                &"username/TestFolder1/TestFolder2/TestFolder3/TestFolder4/test1.text".to_string()
            ));
    }

    #[test]
    fn get_path_tests() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let folder1 = DefaultFileService::create(config, "TestFolder1", root.id, Folder).unwrap();
        let folder2 =
            DefaultFileService::create(config, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 =
            DefaultFileService::create(config, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 =
            DefaultFileService::create(config, "TestFolder4", folder3.id, Folder).unwrap();

        DefaultFileService::create(config, "TestFolder5", folder4.id, Folder).unwrap();
        DefaultFileService::create(config, "test1.text", folder4.id, Document).unwrap();
        DefaultFileService::create(config, "test2.text", folder2.id, Document).unwrap();
        let file = DefaultFileService::create(config, "test3.text", folder2.id, Document).unwrap();
        DefaultFileService::create(config, "test4.text", folder2.id, Document).unwrap();
        DefaultFileService::create(config, "test5.text", folder2.id, Document).unwrap();

        assert!(file_metadata_repo::get_by_path(config, "invalid")
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::get_by_path(
            config,
            "username/TestFolder1/TestFolder2/test3.text",
        )
        .unwrap()
        .is_some());
        assert_eq!(
            file_metadata_repo::get_by_path(config, "username/TestFolder1/TestFolder2/test3.text",)
                .unwrap()
                .unwrap(),
            file
        );

        file_metadata_repo::get_all_paths(config, None)
            .unwrap()
            .into_iter()
            .for_each(|path| {
                assert!(file_metadata_repo::get_by_path(config, &path)
                    .unwrap()
                    .is_some())
            });
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn test_arbitrary_path_file_creation() {
        init_logger(temp_config().path()).expect("Logger failed to initialize in test!");
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let paths_with_empties = ["username//", "username/path//to///file.md"];
        for path in &paths_with_empties {
            let err = DefaultFileService::create_at_path(config, path).unwrap_err();
            assert!(
                matches!(err, NewFileFromPathError::PathContainsEmptyFile),
                "Expected path \"{}\" to return PathContainsEmptyFile but instead it was {:?}",
                path,
                err
            );
        }

        assert!(DefaultFileService::create_at_path(config, "garbage").is_err());
        assert!(DefaultFileService::create_at_path(config, "username/").is_err());
        assert!(DefaultFileService::create_at_path(config, "username/").is_err());
        assert_total_filtered_paths!(config, None, 1);

        assert_eq!(
            DefaultFileService::create_at_path(config, "username/test.txt")
                .unwrap()
                .name,
            "test.txt"
        );
        assert_total_filtered_paths!(config, None, 2);
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 1);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 1);
        assert_total_filtered_paths!(config, Some(FoldersOnly), 1);
        assert_no_metadata_problems!(config);

        assert_eq!(
            DefaultFileService::create_at_path(
                config,
                "username/folder1/folder2/folder3/test2.txt"
            )
            .unwrap()
            .name,
            "test2.txt"
        );
        assert_total_filtered_paths!(config, None, 6);
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 2);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 2);
        assert_no_metadata_problems!(config);

        let file = DefaultFileService::create_at_path(config, "username/folder1/folder2/test3.txt")
            .unwrap();
        assert_total_filtered_paths!(config, None, 7);
        assert_eq!(file.name, "test3.txt");
        assert_eq!(
            file_metadata_repo::get(config, file.parent).unwrap().name,
            "folder2"
        );
        assert_eq!(
            file_metadata_repo::get(config, file.parent)
                .unwrap()
                .file_type,
            Folder
        );
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 3);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 3);

        assert_eq!(
            DefaultFileService::create_at_path(config, "username/folder1/folder2/folder3/folder4/")
                .unwrap()
                .file_type,
            Folder
        );
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 3);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 4);
        assert_total_filtered_paths!(config, Some(FoldersOnly), 5);
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_duplicate_files_via_path() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        DefaultFileService::create_at_path(config, "username/test.txt").unwrap();
        assert!(DefaultFileService::create_at_path(config, "username/test.txt").is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_duplicate_files_via_create() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let file = DefaultFileService::create_at_path(config, "username/test.txt").unwrap();
        assert!(DefaultFileService::create(config, "test.txt", file.parent, Document).is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_document_has_children_via_path() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        DefaultFileService::create_at_path(config, "username/test.txt").unwrap();
        assert!(DefaultFileService::create_at_path(config, "username/test.txt/oops.txt").is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_document_has_children() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let file = DefaultFileService::create_at_path(config, "username/test.txt").unwrap();
        assert!(DefaultFileService::create(config, "oops.txt", file.id, Document).is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_bad_names() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert!(DefaultFileService::create(config, "oops/txt", root.id, Document).is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn rename_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_no_metadata_problems!(config);

        assert!(matches!(
            DefaultFileService::rename_file(config, root.id, "newroot").unwrap_err(),
            DocumentRenameError::CannotRenameRoot
        ));

        let file =
            DefaultFileService::create_at_path(config, "username/folder1/file1.txt").unwrap();
        assert!(
            DefaultLocalChangesRepo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .new
        );
        assert!(
            DefaultLocalChangesRepo::get_local_changes(config, file.parent)
                .unwrap()
                .unwrap()
                .new
        );
        assert_total_local_changes!(config, 2);
        assert_no_metadata_problems!(config);

        DefaultLocalChangesRepo::untrack_new_file(config, file.id).unwrap();
        DefaultLocalChangesRepo::untrack_new_file(config, file.parent).unwrap();
        assert_total_local_changes!(config, 0);

        DefaultFileService::rename_file(config, file.id, "file2.txt").unwrap();
        assert_eq!(
            DefaultLocalChangesRepo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .renamed
                .unwrap()
                .old_value,
            "file1.txt"
        );

        assert_no_metadata_problems!(config);

        DefaultFileService::rename_file(config, file.id, "file23.txt").unwrap();
        assert_total_local_changes!(config, 1);
        assert_eq!(
            DefaultLocalChangesRepo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .renamed
                .unwrap()
                .old_value,
            "file1.txt"
        );
        assert_total_local_changes!(config, 1);

        DefaultFileService::rename_file(config, file.id, "file1.txt").unwrap();
        assert_total_local_changes!(config, 0);
        assert_no_metadata_problems!(config);

        assert!(DefaultFileService::rename_file(config, Uuid::new_v4(), "not_used").is_err());
        assert!(DefaultFileService::rename_file(config, file.id, "file/1.txt").is_err());
        assert_total_local_changes!(config, 0);
        assert_eq!(
            file_metadata_repo::get(config, file.id).unwrap().name,
            "file1.txt"
        );

        let file2 =
            DefaultFileService::create_at_path(config, "username/folder1/file2.txt").unwrap();
        assert_eq!(
            file_metadata_repo::get(config, file2.id).unwrap().name,
            "file2.txt"
        );
        assert!(DefaultFileService::rename_file(config, file2.id, "file1.txt").is_err());
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn move_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_no_metadata_problems!(config);

        assert!(matches!(
            DefaultFileService::move_file(config, root.id, Uuid::new_v4()).unwrap_err(),
            FileMoveError::CannotMoveRoot
        ));

        let file1 =
            DefaultFileService::create_at_path(config, "username/folder1/file.txt").unwrap();
        let og_folder = file1.parent;
        let folder1 = DefaultFileService::create_at_path(config, "username/folder2/").unwrap();
        assert!(
            DefaultFileService::write_document(config, folder1.id, &"should fail".as_bytes(),)
                .is_err()
        );

        assert_no_metadata_problems!(config);

        DefaultFileService::write_document(config, file1.id, "nice doc ;)".as_bytes()).unwrap();

        assert_total_local_changes!(config, 3);
        assert_no_metadata_problems!(config);

        DefaultLocalChangesRepo::untrack_new_file(config, file1.id).unwrap();
        DefaultLocalChangesRepo::untrack_new_file(config, file1.parent).unwrap();
        DefaultLocalChangesRepo::untrack_new_file(config, folder1.id).unwrap();
        assert_total_local_changes!(config, 0);

        DefaultFileService::move_file(config, file1.id, folder1.id).unwrap();

        assert_eq!(
            DefaultFileService::read_document(config, file1.id).unwrap(),
            "nice doc ;)".as_bytes()
        );

        assert_no_metadata_problems!(config);

        assert_eq!(
            file_metadata_repo::get(config, file1.id).unwrap().parent,
            folder1.id
        );
        assert_total_local_changes!(config, 1);

        let file2 =
            DefaultFileService::create_at_path(config, "username/folder3/file.txt").unwrap();
        assert!(DefaultFileService::move_file(config, file1.id, file2.parent).is_err());
        assert!(DefaultFileService::move_file(config, Uuid::new_v4(), file2.parent).is_err());
        assert!(DefaultFileService::move_file(config, file1.id, Uuid::new_v4()).is_err());
        assert_total_local_changes!(config, 3);

        DefaultFileService::move_file(config, file1.id, og_folder).unwrap();
        assert_total_local_changes!(config, 2);
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn test_move_folder_into_itself() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_no_metadata_problems!(config);

        let folder1 = DefaultFileService::create_at_path(config, "username/folder1/").unwrap();
        let folder2 =
            DefaultFileService::create_at_path(config, "username/folder1/folder2/").unwrap();

        assert_total_local_changes!(config, 2);

        assert!(matches!(
            DefaultFileService::move_file(config, folder1.id, folder1.id).unwrap_err(),
            FileMoveError::FolderMovedIntoItself
        ));

        assert!(matches!(
            DefaultFileService::move_file(config, folder1.id, folder2.id).unwrap_err(),
            FileMoveError::FolderMovedIntoItself
        ));
    }

    #[test]
    fn test_keeping_track_of_edits() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let file = DefaultFileService::create_at_path(config, "username/file1.md").unwrap();
        DefaultFileService::write_document(config, file.id, "fresh content".as_bytes()).unwrap();

        assert!(
            DefaultLocalChangesRepo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .new
        );

        DefaultLocalChangesRepo::untrack_new_file(config, file.id).unwrap();
        assert!(DefaultLocalChangesRepo::get_local_changes(config, file.id)
            .unwrap()
            .is_none());
        assert_total_local_changes!(config, 0);

        DefaultFileService::write_document(config, file.id, "fresh content2".as_bytes()).unwrap();
        assert!(DefaultLocalChangesRepo::get_local_changes(config, file.id)
            .unwrap()
            .unwrap()
            .content_edited
            .is_some());
        DefaultFileService::write_document(config, file.id, "fresh content".as_bytes()).unwrap();
        assert!(DefaultLocalChangesRepo::get_local_changes(config, file.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_document_delete_new_documents_no_trace_when_deleted() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let doc1 = DefaultFileService::create(config, "test1.md", root.id, Document).unwrap();

        DefaultFileService::write_document(config, doc1.id, &String::from("content").into_bytes())
            .unwrap();
        DefaultFileService::delete_document(config, doc1.id).unwrap();
        assert_total_local_changes!(config, 0);
        assert!(DefaultLocalChangesRepo::get_local_changes(config, doc1.id)
            .unwrap()
            .is_none());

        assert!(file_metadata_repo::maybe_get(config, doc1.id)
            .unwrap()
            .is_none());

        assert!(DefaultDocumentRepo::maybe_get(config, doc1.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_document_delete_after_sync() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let doc1 = DefaultFileService::create(config, "test1.md", root.id, Document).unwrap();

        DefaultFileService::write_document(config, doc1.id, &String::from("content").into_bytes())
            .unwrap();
        DefaultLocalChangesRepo::delete(config, doc1.id).unwrap();

        DefaultFileService::delete_document(config, doc1.id).unwrap();
        assert_total_local_changes!(config, 1);
        assert!(
            DefaultLocalChangesRepo::get_local_changes(config, doc1.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        assert!(
            file_metadata_repo::maybe_get(config, doc1.id)
                .unwrap()
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn test_folders_are_created_in_order() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        DefaultFileService::create_at_path(config, &format!("{}/a/b/c/d/", account.username))
            .unwrap();
        let folder1 =
            file_metadata_repo::get_by_path(config, &format!("{}/a/b/c/d/", account.username))
                .unwrap()
                .unwrap();
        let folder2 =
            file_metadata_repo::get_by_path(config, &format!("{}/a/b/c/", account.username))
                .unwrap()
                .unwrap();
        let folder3 =
            file_metadata_repo::get_by_path(config, &format!("{}/a/b/", account.username))
                .unwrap()
                .unwrap();
        let folder4 = file_metadata_repo::get_by_path(config, &format!("{}/a/", account.username))
            .unwrap()
            .unwrap();

        assert_eq!(
            DefaultLocalChangesRepo::get_all_local_changes(config)
                .unwrap()
                .into_iter()
                .map(|change| change.id)
                .collect::<Vec<Uuid>>(),
            vec![folder4.id, folder3.id, folder2.id, folder1.id]
        );
    }

    #[test]
    fn test_delete_folder() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let folder1 = DefaultFileService::create(config, "folder1", root.id, Folder).unwrap();
        let document1 = DefaultFileService::create(config, "doc1", folder1.id, Document).unwrap();
        let document2 = DefaultFileService::create(config, "doc2", folder1.id, Document).unwrap();
        let document3 = DefaultFileService::create(config, "doc3", folder1.id, Document).unwrap();

        assert_total_local_changes!(config, 4);

        DefaultFileService::delete_folder(config, folder1.id).unwrap();
        assert_total_local_changes!(config, 1);

        assert!(file_metadata_repo::maybe_get(config, document1.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(config, document2.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(config, document3.id)
            .unwrap()
            .is_none());

        assert!(DefaultDocumentRepo::maybe_get(config, document1.id)
            .unwrap()
            .is_none());
        assert!(DefaultDocumentRepo::maybe_get(config, document2.id)
            .unwrap()
            .is_none());
        assert!(DefaultDocumentRepo::maybe_get(config, document3.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_other_things_are_not_touched_during_delete() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let folder1 = DefaultFileService::create(config, "folder1", root.id, Folder).unwrap();
        DefaultFileService::create(config, "doc1", folder1.id, Document).unwrap();
        DefaultFileService::create(config, "doc2", folder1.id, Document).unwrap();
        DefaultFileService::create(config, "doc3", folder1.id, Document).unwrap();

        let folder2 = DefaultFileService::create(config, "folder2", root.id, Folder).unwrap();
        let document4 = DefaultFileService::create(config, "doc1", folder2.id, Document).unwrap();
        let document5 = DefaultFileService::create(config, "doc2", folder2.id, Document).unwrap();
        let document6 = DefaultFileService::create(config, "doc3", folder2.id, Document).unwrap();

        assert_total_local_changes!(config, 8);

        DefaultFileService::delete_folder(config, folder1.id).unwrap();
        assert_total_local_changes!(config, 5);

        assert!(file_metadata_repo::maybe_get(config, document4.id)
            .unwrap()
            .is_some());
        assert!(file_metadata_repo::maybe_get(config, document5.id)
            .unwrap()
            .is_some());
        assert!(file_metadata_repo::maybe_get(config, document6.id)
            .unwrap()
            .is_some());

        assert!(DefaultDocumentRepo::maybe_get(config, document4.id)
            .unwrap()
            .is_some());
        assert!(DefaultDocumentRepo::maybe_get(config, document5.id)
            .unwrap()
            .is_some());
        assert!(DefaultDocumentRepo::maybe_get(config, document6.id)
            .unwrap()
            .is_some());
    }

    #[test]
    fn test_cannot_delete_root() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        assert!(matches!(
            DefaultFileService::delete_folder(config, root.id).unwrap_err(),
            DeleteFolderError::CannotDeleteRoot
        ));

        assert_total_local_changes!(config, 0);
    }
}
