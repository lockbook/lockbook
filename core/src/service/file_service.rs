use sled::Db;

use crate::model::client_file_metadata::FileType::Folder;
use crate::model::client_file_metadata::{ClientFileMetadata, FileType};
use crate::model::crypto::*;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::{FileMetadataRepo, FindingParentsFailed};
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::FileEncryptionService;
use crate::service::file_service::DocumentUpdateError::{
    CouldNotFindFile, DbError, DocumentWriteError, ThisIsAFolderYouDummy,
};
use crate::service::file_service::NewFileError::{FailedToSaveMetadata, FileCryptoError};
use crate::service::file_service::ReadDocumentError::DocumentReadError;
use serde::export::PhantomData;
use uuid::Uuid;

#[derive(Debug)]
pub enum NewFileError {
    AccountRetrievalError(account_repo::Error),
    CouldNotFindParents(FindingParentsFailed),
    FileCryptoError(file_encryption_service::FileCreationError),
    FailedToSaveMetadata(file_metadata_repo::DbError),
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

pub trait FileService {
    fn create(
        db: &Db,
        name: &str,
        parent: Uuid,
        file_type: FileType,
    ) -> Result<ClientFileMetadata, NewFileError>;

    fn write_document(
        db: &Db,
        id: Uuid,
        content: &DecryptedValue,
    ) -> Result<(), DocumentUpdateError>;

    fn read_document(db: &Db, id: Uuid) -> Result<DecryptedValue, ReadDocumentError>;
}

pub struct FileServiceImpl<
    FileMetadataDb: FileMetadataRepo,
    FileDb: DocumentRepo,
    AccountDb: AccountRepo,
    FileCrypto: FileEncryptionService,
> {
    metadatas: PhantomData<FileMetadataDb>,
    files: PhantomData<FileDb>,
    account: PhantomData<AccountDb>,
    file_crypto: PhantomData<FileCrypto>,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        FileDb: DocumentRepo,
        AccountDb: AccountRepo,
        FileCrypto: FileEncryptionService,
    > FileService for FileServiceImpl<FileMetadataDb, FileDb, AccountDb, FileCrypto>
{
    fn create(
        db: &Db,
        name: &str,
        parent: Uuid,
        file_type: FileType,
    ) -> Result<ClientFileMetadata, NewFileError> {
        let account = AccountDb::get_account(&db).map_err(NewFileError::AccountRetrievalError)?;

        let parents = FileMetadataDb::get_with_all_parents(&db, parent)
            .map_err(NewFileError::CouldNotFindParents)?;

        let new_metadata =
            FileCrypto::create_file_metadata(name, file_type, parent, &account, parents)
                .map_err(FileCryptoError)?;

        FileMetadataDb::insert(&db, &new_metadata).map_err(FailedToSaveMetadata)?;

        Ok(new_metadata)
    }

    fn write_document(
        db: &Db,
        id: Uuid,
        content: &DecryptedValue,
    ) -> Result<(), DocumentUpdateError> {
        let account =
            AccountDb::get_account(&db).map_err(DocumentUpdateError::AccountRetrievalError)?;

        let mut file_metadata = FileMetadataDb::maybe_get(&db, id)
            .map_err(DbError)?
            .ok_or(CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(ThisIsAFolderYouDummy);
        }

        let parents = FileMetadataDb::get_with_all_parents(&db, id)
            .map_err(DocumentUpdateError::CouldNotFindParents)?;

        let new_file = FileCrypto::write_to_document(&account, &content, &file_metadata, parents)
            .map_err(DocumentUpdateError::FileCryptoError)?;

        file_metadata.document_edited = true;

        FileMetadataDb::insert(&db, &file_metadata).map_err(DbError)?;

        FileDb::insert(&db, file_metadata.id, &new_file).map_err(DocumentWriteError)?;

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
    use crate::model::account::Account;
    use crate::model::client_file_metadata::FileType::{Document, Folder};
    use crate::model::crypto::DecryptedValue;
    use crate::model::state::Config;
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::FileMetadataRepo;
    use crate::service::crypto_service::PubKeyCryptoService;
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::service::file_service::FileService;
    use crate::{
        DefaultAccountRepo, DefaultCrypto, DefaultFileEncryptionService, DefaultFileMetadataRepo,
        DefaultFileService,
    };

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

        assert!(DefaultFileMetadataRepo::get_all_paths(&db)
            .unwrap()
            .is_empty());
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db).unwrap().len(),
            1
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db)
                .unwrap()
                .get(0)
                .unwrap(),
            "username/"
        );

        let folder1 = DefaultFileService::create(&db, "TestFolder1", root.id, Folder).unwrap();
        assert_eq!(
            DefaultFileMetadataRepo::get_all_paths(&db).unwrap().len(),
            2
        );
        assert!(DefaultFileMetadataRepo::get_all_paths(&db)
            .unwrap()
            .contains(&"username/".to_string()));
        assert!(DefaultFileMetadataRepo::get_all_paths(&db)
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

        assert!(DefaultFileMetadataRepo::get_all_paths(&db)
            .unwrap()
            .contains(&"username/TestFolder1/TestFolder2/test3.text".to_string()));
        assert!(DefaultFileMetadataRepo::get_all_paths(&db)
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
            "username/TestFolder1/TestFolder2/test3.text"
        )
        .unwrap()
        .is_some());
        assert_eq!(
            DefaultFileMetadataRepo::get_by_path(
                &db,
                "username/TestFolder1/TestFolder2/test3.text"
            )
            .unwrap()
            .unwrap(),
            file
        );

        DefaultFileMetadataRepo::get_all_paths(&db)
            .unwrap()
            .into_iter()
            .for_each(|path| {
                assert!(DefaultFileMetadataRepo::get_by_path(&db, &path)
                    .unwrap()
                    .is_some())
            })
    }
}
