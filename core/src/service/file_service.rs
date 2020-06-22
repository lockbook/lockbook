use sled::Db;

use crate::error_enum;
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
use serde::export::PhantomData;
use uuid::Uuid;

#[derive(Debug)]
pub enum NewFileError {
    AccountRetrievalError(account_repo::Error),
    CouldNotFindParents(FindingParentsFailed),
    FileCryptoError(file_encryption_service::FileCreationError),
    FailedToSaveMetadata(file_metadata_repo::DbError),
}

error_enum! {
    enum UpdateFileError {
        AccountRetrievalError(account_repo::Error),
        FileRetrievalError(file_repo::Error),
        EncryptedWriteError(file_encryption_service::FileWriteError),
        MetadataDbError(file_metadata_repo::Error),

    }
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
    fn update(db: &Db, id: Uuid, content: &str) -> Result<EncryptedFile, UpdateFileError>;
    fn get(db: &Db, id: Uuid) -> Result<DecryptedValue, Error>;
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
        let account = AccountDb::get_account(&db).map_err(AccountRetrievalError)?;

        let parents =
            FileMetadataDb::get_with_all_parents(&db, parent).map_err(CouldNotFindParents)?;

        let new_metadata =
            FileCrypto::create_file_metadata(name, file_type, parent, &account, parents)
                .map_err(FileCryptoError)?;

        FileMetadataDb::insert(&db, &new_metadata).map_err(FailedToSaveMetadata)?;

        Ok(new_metadata)
    }

    fn update(db: &Db, id: Uuid, content: &str) -> Result<EncryptedFile, UpdateFileError> {
        unimplemented!()
    }

    fn get(db: &Db, id: Uuid) -> Result<DecryptedValue, Error> {
        unimplemented!()
    }
}
