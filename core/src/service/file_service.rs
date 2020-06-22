use sled::Db;

use crate::error_enum;
use crate::model::client_file_metadata::FileType::Folder;
use crate::model::client_file_metadata::{ClientFileMetadata, FileType};
use crate::model::crypto::*;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::{FileMetadataRepo, FindingParentsFailed};
use crate::repo::file_repo;
use crate::repo::file_repo::FileRepo;
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::FileEncryptionService;
use crate::service::file_service::NewFileError::{
    AccountRetrievalError, CouldNotFindParents, FailedToSaveMetadata, FileCryptoError,
};
use crate::service::file_service::UpdateFileError::{CouldNotFindFile, DbError, ThisIsAFolderYouDummy, DocumentWriteError};
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
pub enum UpdateFileError {
    AccountRetrievalError(account_repo::Error),
    CouldNotFindFile,
    CouldNotFindParents(FindingParentsFailed),
    ThisIsAFolderYouDummy,
    FileCryptoError(file_encryption_service::FileWriteError),
    DocumentWriteError(file_repo::Error),
    DbError(file_metadata_repo::DbError),
}

error_enum! {
    enum Error {
        FileRepo(file_repo::Error),
        AccountRepo(account_repo::Error),
        EncryptionServiceWrite(file_encryption_service::FileWriteError),
        EncryptionServiceRead(file_encryption_service::UnableToReadFile),
    }
}

pub trait FileService {
    fn create(
        db: &Db,
        name: &str,
        parent: Uuid,
        file_type: FileType,
    ) -> Result<ClientFileMetadata, NewFileError>;

    fn write_document(db: &Db, id: Uuid, content: &DecryptedValue) -> Result<(), UpdateFileError>;

    fn read_document(db: &Db, id: Uuid) -> Result<DecryptedValue, Error>;
}

pub struct FileServiceImpl<
    FileMetadataDb: FileMetadataRepo,
    FileDb: FileRepo,
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
        FileDb: FileRepo,
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

    fn write_document(db: &Db, id: Uuid, content: &DecryptedValue) -> Result<(), UpdateFileError> {
        let account =
            AccountDb::get_account(&db).map_err(UpdateFileError::AccountRetrievalError)?;

        let mut file_metadata = FileMetadataDb::maybe_get(&db, id)
            .map_err(DbError)?
            .ok_or(CouldNotFindFile)?;

        if file_metadata.file_type == Folder {
            return Err(ThisIsAFolderYouDummy);
        }

        let parents = FileMetadataDb::get_with_all_parents(&db, id)
            .map_err(UpdateFileError::CouldNotFindParents)?;

        let new_file = FileCrypto::write_to_document(&account, &content, &file_metadata, parents)
            .map_err(UpdateFileError::FileCryptoError)?;

        file_metadata.document_edited = true;

        FileMetadataDb::insert(&db, &file_metadata)
            .map_err(DbError)?;

        FileDb::insert(&db, file_metadata.id, &new_file)
            .map_err(DocumentWriteError)?;

        Ok(())
    }

    fn read_document(db: &Db, id: Uuid) -> Result<DecryptedValue, Error> {
        unimplemented!()
    }
}
