use serde::export::PhantomData;
use sled::Db;
use uuid::Uuid;

use crate::model::crypto::*;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::model::file_metadata::FileType::{Document, Folder};
use crate::repo::{account_repo, local_changes_repo};
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::{FileMetadataRepo, FindingParentsFailed};
use crate::repo::local_changes_repo::LocalChangesRepo;
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::FileEncryptionService;
use crate::service::file_service::DocumentUpdateError::{
    CouldNotFindFile, DbError, DocumentWriteError, ThisIsAFolderYouDummy,
};
use crate::service::file_service::NewFileError::{
    FailedToSaveMetadata, FailedToWriteFileContent, FileCryptoError,
};
use crate::service::file_service::NewFileFromPathError::{
    FailedToCreateChild, InvalidRootFolder, NoRoot,
};
use crate::service::file_service::ReadDocumentError::DocumentReadError;

#[derive(Debug)]
pub enum NewFileError {
    AccountRetrievalError(account_repo::Error),
    CouldNotFindParents(FindingParentsFailed),
    FileCryptoError(file_encryption_service::FileCreationError),
    FailedToSaveMetadata(file_metadata_repo::DbError),
    FailedToWriteFileContent(DocumentUpdateError),
    FailedToRecordChange(local_changes_repo::DbError),
}

#[derive(Debug)]
pub enum NewFileFromPathError {
    DbError(file_metadata_repo::DbError),
    NoRoot,
    InvalidRootFolder,
    FailedToCreateChild(NewFileError),
    FailedToRecordChange(local_changes_repo::DbError),
}

#[derive(Debug)]
pub enum DocumentUpdateError {
    AccountRetrievalError(account_repo::Error),
    CouldNotFindFile,
    CouldNotFindParents(FindingParentsFailed),
    ThisIsAFolderYouDummy,
    FileCryptoError(file_encryption_service::FileWriteError),
    DocumentWriteError(document_repo::Error),
    DbError(file_metadata_repo::DbError),
    FailedToRecordChange(local_changes_repo::DbError),
}

#[derive(Debug)]
pub enum ReadDocumentError {
    AccountRetrievalError(account_repo::Error),
    CouldNotFindFile,
    DbError(file_metadata_repo::DbError),
    ThisIsAFolderYouDummy,
    DocumentReadError(document_repo::Error),
    CouldNotFindParents(FindingParentsFailed),
    FileCryptoError(file_encryption_service::UnableToReadFile),
}

#[derive(Debug)]
pub enum DocumentRenameError {
    FailedToRecordChange(local_changes_repo::DbError),
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
    //
    // fn rename_file(
    //     db: &Db,
    //     file_metadata: Uuid,
    //     new_name: &str,
    // ) -> Result<(), DocumentRenameError>;
    //
    // fn move_file(
    //     db: &Db,
    //     file_metadata: Uuid,
    //     new_parent: Uuid
    // ) -> Result<(), ()>;

    fn read_document(db: &Db, id: Uuid) -> Result<DecryptedValue, ReadDocumentError>;
}

pub struct FileServiceImpl<
    FileMetadataDb: FileMetadataRepo,
    FileDb: DocumentRepo,
    ChangesDb: LocalChangesRepo,
    AccountDb: AccountRepo,
    FileCrypto: FileEncryptionService,
> {
    metadatas: PhantomData<FileMetadataDb>,
    files: PhantomData<FileDb>,
    changes_db: PhantomData<ChangesDb>,
    account: PhantomData<AccountDb>,
    file_crypto: PhantomData<FileCrypto>,
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
        let account = AccountDb::get_account(&db).map_err(NewFileError::AccountRetrievalError)?;

        let parents = FileMetadataDb::get_with_all_parents(&db, parent)
            .map_err(NewFileError::CouldNotFindParents)?;

        // TODO add test / check here that `parent` is a Folder

        // TODO check here that a node with the same parent does not share a name with this file

        // TODO check that a file with this id doesn't already exist

        let new_metadata =
            FileCrypto::create_file_metadata(name, file_type, parent, &account, parents)
                .map_err(FileCryptoError)?;

        FileMetadataDb::insert(&db, &new_metadata).map_err(FailedToSaveMetadata)?;
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

    // TODO how does passing in the same path twice work?
    // TODO how do folder / document interactions work?
    fn create_at_path(db: &Db, path_and_name: &str) -> Result<FileMetadata, NewFileFromPathError> {
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
            return Err(InvalidRootFolder);
        }

        // We're going to look ahead, and find or create the right child
        'path: for index in 0..path_components.len() - 1 {
            let children = FileMetadataDb::get_children(&db, current.id)
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
                    current = child;
                    continue 'path; // Child exists, onto the next one
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
            return Err(ThisIsAFolderYouDummy);
        }

        let parents = FileMetadataDb::get_with_all_parents(&db, id)
            .map_err(DocumentUpdateError::CouldNotFindParents)?;

        let new_file = FileCrypto::write_to_document(&account, &content, &file_metadata, parents)
            .map_err(DocumentUpdateError::FileCryptoError)?;

        FileMetadataDb::insert(&db, &file_metadata).map_err(DbError)?;
        FileDb::insert(&db, file_metadata.id, &new_file).map_err(DocumentWriteError)?;
        ChangesDb::track_edit(&db, file_metadata.id)
            .map_err(DocumentUpdateError::FailedToRecordChange)?;

        Ok(())
    }

    fn read_document(db: &Db, id: Uuid) -> Result<DecryptedValue, ReadDocumentError> {
        let account =
            AccountDb::get_account(&db).map_err(ReadDocumentError::AccountRetrievalError)?;

        let file_metadata = FileMetadataDb::maybe_get(&db, id)
            .map_err(ReadDocumentError::DbError)?
            .ok_or(ReadDocumentError::CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(ReadDocumentError::ThisIsAFolderYouDummy);
        }

        let document = FileDb::get(&db, id).map_err(DocumentReadError)?;

        let parents = FileMetadataDb::get_with_all_parents(&db, id)
            .map_err(ReadDocumentError::CouldNotFindParents)?;

        let contents = FileCrypto::read_document(&account, &document, &file_metadata, parents)
            .map_err(ReadDocumentError::FileCryptoError)?;

        Ok(contents)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::{
        DefaultAccountRepo, DefaultCrypto, DefaultFileEncryptionService, DefaultFileMetadataRepo,
        DefaultFileService, init_logger_safely,
    };
    use crate::model::account::Account;
    use crate::model::crypto::DecryptedValue;
    use crate::model::file_metadata::FileType::{Document, Folder};
    use crate::model::state::Config;
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::FileMetadataRepo;
    use crate::repo::file_metadata_repo::Filter::{DocumentsOnly, LeafNodesOnly};
    use crate::service::crypto_service::PubKeyCryptoService;
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::service::file_service::FileService;

    #[test]
    fn file_service_runthrough() {
        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = TempBackedDB::connect_to_db(&config).unwrap();
        let keys = DefaultCrypto::generate_key().unwrap();

        let account = Account {
            username: String::from("username"),
            keys,
        };

        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        assert!(DefaultFileMetadataRepo::get_root(&db).unwrap().is_none());
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert!(DefaultFileMetadataRepo::get_root(&db).unwrap().is_some());

        let folder1 = DefaultFileService::create(&db, "TestFolder1", root.id, Folder).unwrap();
        let folder2 = DefaultFileService::create(&db, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 = DefaultFileService::create(&db, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 = DefaultFileService::create(&db, "TestFolder4", folder3.id, Folder).unwrap();
        let folder5 = DefaultFileService::create(&db, "TestFolder5", folder4.id, Folder).unwrap();
        let file = DefaultFileService::create(&db, "test.text", folder5.id, Document).unwrap();

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
    }

    #[test]
    fn path_calculations_runthrough() {
        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = TempBackedDB::connect_to_db(&config).unwrap();
        let keys = DefaultCrypto::generate_key().unwrap();

        let account = Account {
            username: String::from("username"),
            keys,
        };

        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        assert!(DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .is_empty());
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .get(0)
                .unwrap(),
            "username/"
        );

        let folder1 = DefaultFileService::create(&db, "TestFolder1", root.id, Folder).unwrap();
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .len(),
            2
        );
        assert!(DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .contains(&"username/".to_string()));
        assert!(DefaultFileMetadataRepo::get_all_paths(&db, None)
            .unwrap()
            .contains(&"username/TestFolder1/".to_string()));
        let folder2 = DefaultFileService::create(&db, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 = DefaultFileService::create(&db, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 = DefaultFileService::create(&db, "TestFolder4", folder3.id, Folder).unwrap();

        DefaultFileService::create(&db, "TestFolder5", folder4.id, Folder).unwrap();
        DefaultFileService::create(&db, "test1.text", folder4.id, Document).unwrap();
        DefaultFileService::create(&db, "test2.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test3.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test4.text", folder2.id, Document).unwrap();
        DefaultFileService::create(&db, "test5.text", folder2.id, Document).unwrap();

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
        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = TempBackedDB::connect_to_db(&config).unwrap();
        let keys = DefaultCrypto::generate_key().unwrap();

        let account = Account {
            username: String::from("username"),
            keys,
        };

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
            })
    }

    #[test]
    fn test_arbitary_path_file_creation() {
        init_logger_safely();
        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = TempBackedDB::connect_to_db(&config).unwrap();
        let keys = DefaultCrypto::generate_key().unwrap();
        let account = Account {
            username: String::from("username"),
            keys,
        };

        DefaultAccountRepo::insert_account(&db, &account).unwrap();

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        assert!(DefaultFileService::create_at_path(&db, "garbage").is_err());
        assert_eq!(
            DefaultFileService::create_at_path(&db, "username")
                .unwrap()
                .name,
            "username"
        );
        assert_eq!(
            DefaultFileService::create_at_path(&db, "username/")
                .unwrap()
                .name,
            "username"
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            DefaultFileService::create_at_path(&db, "username/test.txt")
                .unwrap()
                .name,
            "test.txt"
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(DocumentsOnly))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(LeafNodesOnly))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            DefaultFileService::create_at_path(&db, "username/folder1/folder2/folder3/test2.txt")
                .unwrap()
                .name,
            "test2.txt"
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(DocumentsOnly))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(LeafNodesOnly))
                .unwrap()
                .len(),
            2
        );
        println!(
            "{:?}",
            DefaultFileMetadataRepo::get_all_paths(&db, None).unwrap()
        );
        let file =
            DefaultFileService::create_at_path(&db, "username/folder1/folder2/test3.txt").unwrap();
        println!(
            "{:?}",
            DefaultFileMetadataRepo::get_all_paths(&db, None).unwrap()
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, None)
                .unwrap()
                .len(),
            7
        );
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
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(DocumentsOnly))
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(LeafNodesOnly))
                .unwrap()
                .len(),
            3
        );

        assert_eq!(
            DefaultFileService::create_at_path(&db, "username/folder1/folder2/folder3/folder4/")
                .unwrap()
                .file_type,
            Folder
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(DocumentsOnly))
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db, Some(LeafNodesOnly))
                .unwrap()
                .len(),
            4
        );
    }
}
