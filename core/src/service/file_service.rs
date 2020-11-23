use sha2::{Digest, Sha256};
use sled::Db;
use uuid::Uuid;

use crate::model::crypto::*;
use crate::model::file_metadata::FileType::{Document, Folder};
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::{FileMetadataRepo, FindingParentsFailed};
use crate::repo::local_changes_repo::LocalChangesRepo;
use crate::repo::{account_repo, local_changes_repo};
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
use crate::DefaultFileMetadataRepo;

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
        db: &Db,
        name: &str,
        parent: Uuid,
        file_type: FileType,
    ) -> Result<FileMetadata, NewFileError>;

    fn create_at_path(db: &Db, path_and_name: &str) -> Result<FileMetadata, NewFileFromPathError>;

    fn write_document(
        db: &Db,
        id: Uuid,
        content: &DecryptedValue,
    ) -> Result<(), DocumentUpdateError>;

    fn rename_file(db: &Db, id: Uuid, new_name: &str) -> Result<(), DocumentRenameError>;

    fn move_file(db: &Db, file_metadata: Uuid, new_parent: Uuid) -> Result<(), FileMoveError>;

    fn read_document(db: &Db, id: Uuid) -> Result<DecryptedValue, ReadDocumentError>;

    fn delete_document(db: &Db, id: Uuid) -> Result<(), DeleteDocumentError>;

    fn delete_folder(db: &Db, id: Uuid) -> Result<(), DeleteFolderError>;
}

pub struct FileServiceImpl<
    FileMetadataDb: FileMetadataRepo,
    FileDb: DocumentRepo,
    ChangesDb: LocalChangesRepo,
    AccountDb: AccountRepo,
    FileCrypto: FileEncryptionService,
> {
    _metadatas: FileMetadataDb,
    _files: FileDb,
    _changes_db: ChangesDb,
    _account: AccountDb,
    _file_crypto: FileCrypto,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        FileDb: DocumentRepo,
        ChangesDb: LocalChangesRepo,
        AccountDb: AccountRepo,
        FileCrypto: FileEncryptionService,
    > FileService for FileServiceImpl<FileMetadataDb, FileDb, ChangesDb, AccountDb, FileCrypto>
{
    fn create(
        db: &Db,
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

        let account = AccountDb::get_account(&db).map_err(NewFileError::AccountRetrievalError)?;

        let parents = FileMetadataDb::get_with_all_parents(&db, parent)
            .map_err(NewFileError::CouldNotFindParents)?;

        // Make sure parent is in fact a folder
        if let Some(parent) = parents.get(&parent) {
            if parent.file_type == Document {
                return Err(DocumentTreatedAsFolder);
            }
        }

        // Check that this file name is available
        for child in DefaultFileMetadataRepo::get_children_non_recursively(&db, parent)
            .map_err(MetadataRepoError)?
        {
            if child.name == name {
                return Err(FileNameNotAvailable);
            }
        }

        let new_metadata =
            FileCrypto::create_file_metadata(name, file_type, parent, &account, parents)
                .map_err(FileCryptoError)?;

        FileMetadataDb::insert(&db, &new_metadata).map_err(MetadataRepoError)?;
        ChangesDb::track_new_file(&db, new_metadata.id)
            .map_err(NewFileError::FailedToRecordChange)?;

        if file_type == Document {
            Self::write_document(
                &db,
                new_metadata.id,
                &DecryptedValue {
                    secret: "".to_string(),
                },
            )
            .map_err(FailedToWriteFileContent)?;
        }
        Ok(new_metadata)
    }

    fn create_at_path(db: &Db, path_and_name: &str) -> Result<FileMetadata, NewFileFromPathError> {
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

        let mut current = FileMetadataDb::get_root(&db)
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
            let children = FileMetadataDb::get_children_non_recursively(&db, current.id)
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

            current =
                Self::create(&db, next_name, current.id, file_type).map_err(FailedToCreateChild)?;
        }

        Ok(current)
    }

    fn write_document(
        db: &Db,
        id: Uuid,
        content: &DecryptedValue,
    ) -> Result<(), DocumentUpdateError> {
        let account =
            AccountDb::get_account(&db).map_err(DocumentUpdateError::AccountRetrievalError)?;

        let file_metadata = FileMetadataDb::maybe_get(&db, id)
            .map_err(DbError)?
            .ok_or(CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(FolderTreatedAsDocument);
        }

        let parents = FileMetadataDb::get_with_all_parents(&db, id)
            .map_err(DocumentUpdateError::CouldNotFindParents)?;

        let new_file =
            FileCrypto::write_to_document(&account, &content, &file_metadata, parents.clone())
                .map_err(DocumentUpdateError::FileCryptoError)?;

        FileMetadataDb::insert(&db, &file_metadata).map_err(DbError)?;

        if let Some(old_encrypted) = FileDb::maybe_get(&db, id).map_err(FetchOldVersionError)? {
            let decrypted = FileCrypto::read_document(
                &account,
                &old_encrypted,
                &file_metadata,
                parents.clone(),
            )
            .map_err(DocumentUpdateError::DecryptOldVersionError)?;

            let permanent_access_info = FileCrypto::get_key_for_user(&account, id, parents)
                .map_err(AccessInfoCreationError)?;

            ChangesDb::track_edit(
                &db,
                file_metadata.id,
                &old_encrypted,
                &permanent_access_info,
                Sha256::digest(decrypted.secret.as_bytes()).to_vec(),
                Sha256::digest(content.secret.as_bytes()).to_vec(),
            )
            .map_err(DocumentUpdateError::FailedToRecordChange)?;
        };

        FileDb::insert(&db, file_metadata.id, &new_file).map_err(DocumentWriteError)?;

        Ok(())
    }

    fn rename_file(db: &Db, id: Uuid, new_name: &str) -> Result<(), DocumentRenameError> {
        if new_name.is_empty() {
            return Err(DocumentRenameError::FileNameEmpty);
        }

        if new_name.contains('/') {
            return Err(DocumentRenameError::FileNameContainsSlash);
        }

        match FileMetadataDb::maybe_get(&db, id).map_err(DocumentRenameError::DbError)? {
            None => Err(FileDoesNotExist),
            Some(mut file) => {
                if file.id == file.parent {
                    return Err(CannotRenameRoot);
                }

                let siblings = FileMetadataDb::get_children_non_recursively(&db, file.parent)
                    .map_err(DocumentRenameError::DbError)?;

                // Check that this file name is available
                for child in siblings {
                    if child.name == new_name {
                        return Err(DocumentRenameError::FileNameNotAvailable);
                    }
                }

                ChangesDb::track_rename(&db, file.id, &file.name, new_name)
                    .map_err(DocumentRenameError::FailedToRecordChange)?;

                file.name = new_name.to_string();
                FileMetadataDb::insert(&db, &file).map_err(DocumentRenameError::DbError)?;

                Ok(())
            }
        }
    }

    fn move_file(db: &Db, id: Uuid, new_parent: Uuid) -> Result<(), FileMoveError> {
        let account = AccountDb::get_account(&db).map_err(FileMoveError::AccountRetrievalError)?;

        if id == new_parent {
            return Err(FileMoveError::FolderMovedIntoItself);
        }

        match FileMetadataDb::maybe_get(&db, id).map_err(FileMoveError::DbError)? {
            None => Err(FileDNE),
            Some(mut file) => {
                if file.id == file.parent {
                    return Err(CannotMoveRoot);
                }

                match FileMetadataDb::maybe_get(&db, new_parent).map_err(FileMoveError::DbError)? {
                    None => Err(TargetParentDoesNotExist),
                    Some(parent_metadata) => {
                        if parent_metadata.file_type == Document {
                            return Err(FileMoveError::DocumentTreatedAsFolder);
                        }

                        let siblings =
                            FileMetadataDb::get_children_non_recursively(&db, parent_metadata.id)
                                .map_err(FileMoveError::DbError)?;

                        // Check that this file name is available
                        for child in siblings {
                            if child.name == file.name {
                                return Err(FileMoveError::TargetParentHasChildNamedThat);
                            }
                        }

                        // Good to move
                        let old_parents = FileMetadataDb::get_with_all_parents(&db, file.id)
                            .map_err(FileMoveError::CouldNotFindParents)?;

                        let access_key =
                            FileCrypto::decrypt_key_for_file(&account, file.id, old_parents)
                                .map_err(FailedToDecryptKey)?;

                        let new_parents =
                            FileMetadataDb::get_with_all_parents(&db, parent_metadata.id)
                                .map_err(FileMoveError::CouldNotFindParents)?;

                        let new_access_info = FileCrypto::re_encrypt_key_for_file(
                            &account,
                            access_key,
                            parent_metadata.id,
                            new_parents,
                        )
                        .map_err(FailedToReEncryptKey)?;

                        ChangesDb::track_move(&db, file.id, file.parent, parent_metadata.id)
                            .map_err(FileMoveError::FailedToRecordChange)?;
                        file.parent = parent_metadata.id;
                        file.folder_access_keys = new_access_info;

                        FileMetadataDb::insert(&db, &file).map_err(FileMoveError::DbError)?;
                        Ok(())
                    }
                }
            }
        }
    }

    fn read_document(db: &Db, id: Uuid) -> Result<DecryptedValue, ReadDocumentError> {
        let account =
            AccountDb::get_account(&db).map_err(ReadDocumentError::AccountRetrievalError)?;

        let file_metadata = FileMetadataDb::maybe_get(&db, id)
            .map_err(ReadDocumentError::DbError)?
            .ok_or(ReadDocumentError::CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(ReadDocumentError::TreatedFolderAsDocument);
        }

        let document = FileDb::get(&db, id).map_err(DocumentReadError)?;

        let parents = FileMetadataDb::get_with_all_parents(&db, id)
            .map_err(ReadDocumentError::CouldNotFindParents)?;

        let contents = FileCrypto::read_document(&account, &document, &file_metadata, parents)
            .map_err(ReadDocumentError::FileCryptoError)?;

        Ok(contents)
    }

    fn delete_document(db: &Db, id: Uuid) -> Result<(), DeleteDocumentError> {
        let mut file_metadata = FileMetadataDb::maybe_get(&db, id)
            .map_err(DeleteDocumentError::DbError)?
            .ok_or(DeleteDocumentError::CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(DeleteDocumentError::FolderTreatedAsDocument);
        }

        let new = if let Some(change) = ChangesDb::get_local_changes(&db, id)
            .map_err(DeleteDocumentError::FailedToTrackDelete)?
        {
            change.new
        } else {
            false
        };

        if !new {
            file_metadata.deleted = true;
            FileMetadataDb::insert(&db, &file_metadata)
                .map_err(DeleteDocumentError::FailedToUpdateMetadata)?;
        } else {
            FileMetadataDb::non_recursive_delete_if_exists(&db, id)
                .map_err(DeleteDocumentError::FailedToUpdateMetadata)?;
        }

        FileDb::delete_if_exists(&db, id).map_err(DeleteDocumentError::FailedToDeleteDocument)?;
        ChangesDb::track_delete(&db, id, file_metadata.file_type)
            .map_err(DeleteDocumentError::FailedToTrackDelete)?;

        Ok(())
    }

    fn delete_folder(db: &Db, id: Uuid) -> Result<(), DeleteFolderError> {
        let file_metadata = FileMetadataDb::maybe_get(&db, id)
            .map_err(DeleteFolderError::MetadataError)?
            .ok_or(DeleteFolderError::CouldNotFindFile)?;

        if file_metadata.id == file_metadata.parent {
            return Err(DeleteFolderError::CannotDeleteRoot);
        }

        if file_metadata.file_type == Document {
            return Err(DeleteFolderError::DocumentTreatedAsFolder);
        }

        ChangesDb::track_delete(&db, id, file_metadata.file_type)
            .map_err(DeleteFolderError::FailedToRecordChange)?;

        let files_to_delete = FileMetadataDb::get_and_get_children_recursively(&db, id)
            .map_err(DeleteFolderError::FindingChildrenFailed)?;

        // Server has told us we have the most recent version of all children in this directory and that we can delete now
        for mut file in files_to_delete {
            if file.file_type == Document {
                FileDb::delete_if_exists(&db, file.id)
                    .map_err(DeleteFolderError::FailedToDeleteDocument)?;
            }

            let moved = if let Some(change) = ChangesDb::get_local_changes(&db, file.id)
                .map_err(DeleteFolderError::FailedToDeleteChangeEntry)?
            {
                change.moved.is_some()
            } else {
                false
            };

            if file.id != id && !moved {
                FileMetadataDb::non_recursive_delete_if_exists(&db, file.id)
                    .map_err(DeleteFolderError::FailedToDeleteMetadata)?;

                ChangesDb::delete_if_exists(&db, file.id)
                    .map_err(DeleteFolderError::FailedToDeleteChangeEntry)?;
            } else {
                file.deleted = true;
                FileMetadataDb::insert(&db, &file)
                    .map_err(DeleteFolderError::FailedToDeleteMetadata)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::account::Account;
    use crate::model::crypto::DecryptedValue;
    use crate::model::file_metadata::FileType::{Document, Folder};
    use crate::model::state::dummy_config;
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::document_repo::DocumentRepo;
    use crate::repo::file_metadata_repo::FileMetadataRepo;
    use crate::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
    use crate::repo::local_changes_repo::LocalChangesRepo;
    use crate::service::crypto_service::PubKeyCryptoService;
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::service::file_service::{
        DocumentRenameError, FileMoveError, FileService, NewFileError,
    };
    use crate::{
        init_logger, DefaultAccountRepo, DefaultCrypto, DefaultDocumentRepo,
        DefaultFileEncryptionService, DefaultFileMetadataRepo, DefaultFileService,
        DefaultLocalChangesRepo, NewFileFromPathError,
    };

    type DefaultDbProvider = TempBackedDB;

    macro_rules! assert_no_metadata_problems (
        ($db:expr) => {
            assert!(DefaultFileMetadataRepo::test_repo_integrity($db)
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
                DefaultFileMetadataRepo::get_all_paths($db, $filter)
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
            keys: DefaultCrypto::generate_key().unwrap(),
        }
    }

    #[test]
    fn file_service_runthrough() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        assert!(DefaultFileMetadataRepo::get_root(&db).unwrap().is_none());

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert!(DefaultFileMetadataRepo::get_root(&db).unwrap().is_some());
        assert_no_metadata_problems!(&db);

        assert!(matches!(
            DefaultFileService::create(&db, "", root.id, Document).unwrap_err(),
            NewFileError::FileNameEmpty
        ));

        let folder1 = DefaultFileService::create(&db, "TestFolder1", root.id, Folder).unwrap();
        assert_no_metadata_problems!(&db);

        let folder2 = DefaultFileService::create(&db, "TestFolder2", folder1.id, Folder).unwrap();
        assert_no_metadata_problems!(&db);

        let folder3 = DefaultFileService::create(&db, "TestFolder3", folder2.id, Folder).unwrap();
        assert_no_metadata_problems!(&db);

        let folder4 = DefaultFileService::create(&db, "TestFolder4", folder3.id, Folder).unwrap();
        assert_no_metadata_problems!(&db);

        let folder5 = DefaultFileService::create(&db, "TestFolder5", folder4.id, Folder).unwrap();
        assert_no_metadata_problems!(&db);

        let file = DefaultFileService::create(&db, "test.text", folder5.id, Document).unwrap();
        assert_no_metadata_problems!(&db);

        assert_total_filtered_paths!(&db, Some(FoldersOnly), 6);
        assert_total_filtered_paths!(&db, Some(LeafNodesOnly), 1);
        assert_total_filtered_paths!(&db, Some(DocumentsOnly), 1);

        DefaultFileService::write_document(
            &db,
            file.id,
            &DecryptedValue {
                secret: "5 folders deep".to_string(),
            },
        )
        .unwrap();

        assert_eq!(
            DefaultFileService::read_document(&db, file.id)
                .unwrap()
                .secret,
            "5 folders deep".to_string()
        );
        assert!(DefaultFileService::read_document(&db, folder4.id).is_err());
        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn path_calculations_runthrough() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        assert_total_filtered_paths!(&db, None, 0);

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert_total_filtered_paths!(&db, None, 1);
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .get(0)
                .unwrap(),
            "username/"
        );

        assert_no_metadata_problems!(&db);

        let folder1 = DefaultFileService::create(&db, "TestFolder1", root.id, Folder).unwrap();
        assert_total_filtered_paths!(&db, None, 2);
        assert!(DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .contains(&"username/".to_string()));
        assert!(DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .contains(&"username/TestFolder1/".to_string()));

        assert_no_metadata_problems!(&db);

        let folder2 = DefaultFileService::create(&db, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 = DefaultFileService::create(&db, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 = DefaultFileService::create(&db, "TestFolder4", folder3.id, Folder).unwrap();

        DefaultFileService::create(&db, "TestFolder5", folder4.id, Folder).unwrap();
        DefaultFileService::create(&db, "test1.text", folder4.id, Document).unwrap();
        DefaultFileService::create(&db, "test2.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test3.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test4.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test5.text", folder2.id, Document).unwrap();
        assert_no_metadata_problems!(&db);

        assert!(DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .contains(&"username/TestFolder1/TestFolder2/test3.text".to_string()));
        assert!(DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .contains(
                &"username/TestFolder1/TestFolder2/TestFolder3/TestFolder4/test1.text".to_string()
            ));
    }

    #[test]
    fn get_path_tests() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let folder1 = DefaultFileService::create(&db, "TestFolder1", root.id, Folder).unwrap();
        let folder2 = DefaultFileService::create(&db, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 = DefaultFileService::create(&db, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 = DefaultFileService::create(&db, "TestFolder4", folder3.id, Folder).unwrap();

        DefaultFileService::create(&db, "TestFolder5", folder4.id, Folder).unwrap();
        DefaultFileService::create(&db, "test1.text", folder4.id, Document).unwrap();
        DefaultFileService::create(&db, "test2.text", folder2.id, Document).unwrap();
        let file = DefaultFileService::create(&db, "test3.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test4.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test5.text", folder2.id, Document).unwrap();

        assert!(DefaultFileMetadataRepo::get_by_path(&db, "invalid")
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::get_by_path(
            &db,
            "username/TestFolder1/TestFolder2/test3.text",
        )
        .unwrap()
        .is_some());
        assert_eq!(
            DefaultFileMetadataRepo::get_by_path(
                &db,
                "username/TestFolder1/TestFolder2/test3.text",
            )
            .unwrap()
            .unwrap(),
            file
        );

        DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .into_iter()
            .for_each(|path| {
                assert!(DefaultFileMetadataRepo::get_by_path(&db, &path)
                    .unwrap()
                    .is_some())
            });
        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn test_arbitrary_path_file_creation() {
        init_logger(dummy_config().path()).expect("Logger failed to initialize in test!");
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let paths_with_empties = ["username//", "username/path//to///file.md"];
        for path in &paths_with_empties {
            let err = DefaultFileService::create_at_path(&db, path).unwrap_err();
            assert!(
                matches!(err, NewFileFromPathError::PathContainsEmptyFile),
                "Expected path \"{}\" to return PathContainsEmptyFile but instead it was {:?}",
                path,
                err
            );
        }

        assert!(DefaultFileService::create_at_path(&db, "garbage").is_err());
        assert!(DefaultFileService::create_at_path(&db, "username/").is_err());
        assert!(DefaultFileService::create_at_path(&db, "username/").is_err());
        assert_total_filtered_paths!(&db, None, 1);

        assert_eq!(
            DefaultFileService::create_at_path(&db, "username/test.txt")
                .unwrap()
                .name,
            "test.txt"
        );
        assert_total_filtered_paths!(&db, None, 2);
        assert_total_filtered_paths!(&db, Some(DocumentsOnly), 1);
        assert_total_filtered_paths!(&db, Some(LeafNodesOnly), 1);
        assert_total_filtered_paths!(&db, Some(FoldersOnly), 1);
        assert_no_metadata_problems!(&db);

        assert_eq!(
            DefaultFileService::create_at_path(&db, "username/folder1/folder2/folder3/test2.txt")
                .unwrap()
                .name,
            "test2.txt"
        );
        assert_total_filtered_paths!(&db, None, 6);
        assert_total_filtered_paths!(&db, Some(DocumentsOnly), 2);
        assert_total_filtered_paths!(&db, Some(LeafNodesOnly), 2);
        assert_no_metadata_problems!(&db);

        let file =
            DefaultFileService::create_at_path(&db, "username/folder1/folder2/test3.txt").unwrap();
        assert_total_filtered_paths!(&db, None, 7);
        assert_eq!(file.name, "test3.txt");
        assert_eq!(
            DefaultFileMetadataRepo::get(&db, file.parent).unwrap().name,
            "folder2"
        );
        assert_eq!(
            DefaultFileMetadataRepo::get(&db, file.parent)
                .unwrap()
                .file_type,
            Folder
        );
        assert_total_filtered_paths!(&db, Some(DocumentsOnly), 3);
        assert_total_filtered_paths!(&db, Some(LeafNodesOnly), 3);

        assert_eq!(
            DefaultFileService::create_at_path(&db, "username/folder1/folder2/folder3/folder4/")
                .unwrap()
                .file_type,
            Folder
        );
        assert_total_filtered_paths!(&db, Some(DocumentsOnly), 3);
        assert_total_filtered_paths!(&db, Some(LeafNodesOnly), 4);
        assert_total_filtered_paths!(&db, Some(FoldersOnly), 5);
        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn ensure_no_duplicate_files_via_path() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        DefaultFileService::create_at_path(&db, "username/test.txt").unwrap();
        assert!(DefaultFileService::create_at_path(&db, "username/test.txt").is_err());

        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn ensure_no_duplicate_files_via_create() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let file = DefaultFileService::create_at_path(&db, "username/test.txt").unwrap();
        assert!(DefaultFileService::create(&db, "test.txt", file.parent, Document).is_err());

        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn ensure_no_document_has_children_via_path() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        DefaultFileService::create_at_path(&db, "username/test.txt").unwrap();
        assert!(DefaultFileService::create_at_path(&db, "username/test.txt/oops.txt").is_err());

        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn ensure_no_document_has_children() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let file = DefaultFileService::create_at_path(&db, "username/test.txt").unwrap();
        assert!(DefaultFileService::create(&db, "oops.txt", file.id, Document).is_err());

        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn ensure_no_bad_names() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert!(DefaultFileService::create(&db, "oops/txt", root.id, Document).is_err());

        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn rename_runthrough() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert_no_metadata_problems!(&db);

        assert!(matches!(
            DefaultFileService::rename_file(&db, root.id, "newroot").unwrap_err(),
            DocumentRenameError::CannotRenameRoot
        ));

        let file = DefaultFileService::create_at_path(&db, "username/folder1/file1.txt").unwrap();
        assert!(
            DefaultLocalChangesRepo::get_local_changes(&db, file.id)
                .unwrap()
                .unwrap()
                .new
        );
        assert!(
            DefaultLocalChangesRepo::get_local_changes(&db, file.parent)
                .unwrap()
                .unwrap()
                .new
        );
        assert_total_local_changes!(&db, 2);
        assert_no_metadata_problems!(&db);

        DefaultLocalChangesRepo::untrack_new_file(&db, file.id).unwrap();
        DefaultLocalChangesRepo::untrack_new_file(&db, file.parent).unwrap();
        assert_total_local_changes!(&db, 0);

        DefaultFileService::rename_file(&db, file.id, "file2.txt").unwrap();
        assert_eq!(
            DefaultLocalChangesRepo::get_local_changes(&db, file.id)
                .unwrap()
                .unwrap()
                .renamed
                .unwrap()
                .old_value,
            "file1.txt"
        );

        assert_no_metadata_problems!(&db);

        DefaultFileService::rename_file(&db, file.id, "file23.txt").unwrap();
        assert_total_local_changes!(&db, 1);
        assert_eq!(
            DefaultLocalChangesRepo::get_local_changes(&db, file.id)
                .unwrap()
                .unwrap()
                .renamed
                .unwrap()
                .old_value,
            "file1.txt"
        );
        assert_total_local_changes!(&db, 1);

        DefaultFileService::rename_file(&db, file.id, "file1.txt").unwrap();
        assert_total_local_changes!(&db, 0);
        assert_no_metadata_problems!(&db);

        assert!(DefaultFileService::rename_file(&db, Uuid::new_v4(), "not_used").is_err());
        assert!(DefaultFileService::rename_file(&db, file.id, "file/1.txt").is_err());
        assert_total_local_changes!(&db, 0);
        assert_eq!(
            DefaultFileMetadataRepo::get(&db, file.id).unwrap().name,
            "file1.txt"
        );

        let file2 = DefaultFileService::create_at_path(&db, "username/folder1/file2.txt").unwrap();
        assert_eq!(
            DefaultFileMetadataRepo::get(&db, file2.id).unwrap().name,
            "file2.txt"
        );
        assert!(DefaultFileService::rename_file(&db, file2.id, "file1.txt").is_err());
        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn move_runthrough() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert_no_metadata_problems!(&db);

        assert!(matches!(
            DefaultFileService::move_file(&db, root.id, Uuid::new_v4()).unwrap_err(),
            FileMoveError::CannotMoveRoot
        ));

        let file1 = DefaultFileService::create_at_path(&db, "username/folder1/file.txt").unwrap();
        let og_folder = file1.parent;
        let folder1 = DefaultFileService::create_at_path(&db, "username/folder2/").unwrap();
        assert!(DefaultFileService::write_document(
            &db,
            folder1.id,
            &DecryptedValue::from("should fail"),
        )
        .is_err());

        assert_no_metadata_problems!(&db);

        DefaultFileService::write_document(&db, file1.id, &DecryptedValue::from("nice doc ;)"))
            .unwrap();

        assert_total_local_changes!(&db, 3);
        assert_no_metadata_problems!(&db);

        DefaultLocalChangesRepo::untrack_new_file(&db, file1.id).unwrap();
        DefaultLocalChangesRepo::untrack_new_file(&db, file1.parent).unwrap();
        DefaultLocalChangesRepo::untrack_new_file(&db, folder1.id).unwrap();
        assert_total_local_changes!(&db, 0);

        DefaultFileService::move_file(&db, file1.id, folder1.id).unwrap();

        assert_eq!(
            DefaultFileService::read_document(&db, file1.id)
                .unwrap()
                .secret,
            "nice doc ;)"
        );

        assert_no_metadata_problems!(&db);

        assert_eq!(
            DefaultFileMetadataRepo::get(&db, file1.id).unwrap().parent,
            folder1.id
        );
        assert_total_local_changes!(&db, 1);

        let file2 = DefaultFileService::create_at_path(&db, "username/folder3/file.txt").unwrap();
        assert!(DefaultFileService::move_file(&db, file1.id, file2.parent).is_err());
        assert!(DefaultFileService::move_file(&db, Uuid::new_v4(), file2.parent).is_err());
        assert!(DefaultFileService::move_file(&db, file1.id, Uuid::new_v4()).is_err());
        assert_total_local_changes!(&db, 3);

        DefaultFileService::move_file(&db, file1.id, og_folder).unwrap();
        assert_total_local_changes!(&db, 2);
        assert_no_metadata_problems!(&db);
    }

    #[test]
    fn test_keeping_track_of_edits() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let file = DefaultFileService::create_at_path(&db, "username/file1.md").unwrap();
        DefaultFileService::write_document(&db, file.id, &DecryptedValue::from("fresh content"))
            .unwrap();

        assert!(
            DefaultLocalChangesRepo::get_local_changes(&db, file.id)
                .unwrap()
                .unwrap()
                .new
        );

        DefaultLocalChangesRepo::untrack_new_file(&db, file.id).unwrap();
        assert!(DefaultLocalChangesRepo::get_local_changes(&db, file.id)
            .unwrap()
            .is_none());
        assert_total_local_changes!(&db, 0);

        DefaultFileService::write_document(&db, file.id, &DecryptedValue::from("fresh content2"))
            .unwrap();
        assert!(DefaultLocalChangesRepo::get_local_changes(&db, file.id)
            .unwrap()
            .unwrap()
            .content_edited
            .is_some());
        DefaultFileService::write_document(&db, file.id, &DecryptedValue::from("fresh content"))
            .unwrap();
        assert!(DefaultLocalChangesRepo::get_local_changes(&db, file.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_document_delete_new_documents_no_trace_when_deleted() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let doc1 = DefaultFileService::create(&db, "test1.md", root.id, Document).unwrap();

        DefaultFileService::write_document(&db, doc1.id, &DecryptedValue::from("content")).unwrap();
        DefaultFileService::delete_document(&db, doc1.id).unwrap();
        assert_total_local_changes!(&db, 0);
        assert!(DefaultLocalChangesRepo::get_local_changes(&db, doc1.id)
            .unwrap()
            .is_none());

        assert!(DefaultFileMetadataRepo::maybe_get(&db, doc1.id)
            .unwrap()
            .is_none());

        assert!(DefaultDocumentRepo::maybe_get(&db, doc1.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_document_delete_after_sync() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let doc1 = DefaultFileService::create(&db, "test1.md", root.id, Document).unwrap();

        DefaultFileService::write_document(&db, doc1.id, &DecryptedValue::from("content")).unwrap();
        DefaultLocalChangesRepo::delete_if_exists(&db, doc1.id).unwrap();

        DefaultFileService::delete_document(&db, doc1.id).unwrap();
        assert_total_local_changes!(&db, 1);
        assert!(
            DefaultLocalChangesRepo::get_local_changes(&db, doc1.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        assert!(
            DefaultFileMetadataRepo::maybe_get(&db, doc1.id)
                .unwrap()
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn test_folders_are_created_in_order() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        DefaultFileService::create_at_path(&db, &format!("{}/a/b/c/d/", account.username)).unwrap();
        let folder1 =
            DefaultFileMetadataRepo::get_by_path(&db, &format!("{}/a/b/c/d/", account.username))
                .unwrap()
                .unwrap();
        let folder2 =
            DefaultFileMetadataRepo::get_by_path(&db, &format!("{}/a/b/c/", account.username))
                .unwrap()
                .unwrap();
        let folder3 =
            DefaultFileMetadataRepo::get_by_path(&db, &format!("{}/a/b/", account.username))
                .unwrap()
                .unwrap();
        let folder4 =
            DefaultFileMetadataRepo::get_by_path(&db, &format!("{}/a/", account.username))
                .unwrap()
                .unwrap();

        assert_eq!(
            DefaultLocalChangesRepo::get_all_local_changes(&db)
                .unwrap()
                .into_iter()
                .map(|change| change.id)
                .collect::<Vec<Uuid>>(),
            vec![folder4.id, folder3.id, folder2.id, folder1.id]
        );
    }

    #[test]
    fn test_delete_folder() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let folder1 = DefaultFileService::create(&db, "folder1", root.id, Folder).unwrap();
        let document1 = DefaultFileService::create(&db, "doc1", folder1.id, Document).unwrap();
        let document2 = DefaultFileService::create(&db, "doc2", folder1.id, Document).unwrap();
        let document3 = DefaultFileService::create(&db, "doc3", folder1.id, Document).unwrap();

        assert_total_local_changes!(&db, 4);

        DefaultFileService::delete_folder(&db, folder1.id).unwrap();
        assert_total_local_changes!(&db, 1);

        assert!(DefaultFileMetadataRepo::maybe_get(&db, document1.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db, document2.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db, document3.id)
            .unwrap()
            .is_none());

        assert!(DefaultDocumentRepo::maybe_get(&db, document1.id)
            .unwrap()
            .is_none());
        assert!(DefaultDocumentRepo::maybe_get(&db, document2.id)
            .unwrap()
            .is_none());
        assert!(DefaultDocumentRepo::maybe_get(&db, document3.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_other_things_are_not_touched_during_delete() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let account = test_account();
        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        let folder1 = DefaultFileService::create(&db, "folder1", root.id, Folder).unwrap();
        DefaultFileService::create(&db, "doc1", folder1.id, Document).unwrap();
        DefaultFileService::create(&db, "doc2", folder1.id, Document).unwrap();
        DefaultFileService::create(&db, "doc3", folder1.id, Document).unwrap();

        let folder2 = DefaultFileService::create(&db, "folder2", root.id, Folder).unwrap();
        let document4 = DefaultFileService::create(&db, "doc1", folder2.id, Document).unwrap();
        let document5 = DefaultFileService::create(&db, "doc2", folder2.id, Document).unwrap();
        let document6 = DefaultFileService::create(&db, "doc3", folder2.id, Document).unwrap();

        assert_total_local_changes!(&db, 8);

        DefaultFileService::delete_folder(&db, folder1.id).unwrap();
        assert_total_local_changes!(&db, 5);

        assert!(DefaultFileMetadataRepo::maybe_get(&db, document4.id)
            .unwrap()
            .is_some());
        assert!(DefaultFileMetadataRepo::maybe_get(&db, document5.id)
            .unwrap()
            .is_some());
        assert!(DefaultFileMetadataRepo::maybe_get(&db, document6.id)
            .unwrap()
            .is_some());

        assert!(DefaultDocumentRepo::maybe_get(&db, document4.id)
            .unwrap()
            .is_some());
        assert!(DefaultDocumentRepo::maybe_get(&db, document5.id)
            .unwrap()
            .is_some());
        assert!(DefaultDocumentRepo::maybe_get(&db, document6.id)
            .unwrap()
            .is_some());
    }
}
