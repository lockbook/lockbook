use sled::Db;

use crate::error_enum;
use crate::model::client_file_metadata::{ClientFileMetadata, FileType};
use crate::model::crypto::*;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo;
use crate::repo::file_repo::FileRepo;
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::FileEncryptionService;
use serde::export::PhantomData;
use uuid::Uuid;

error_enum! {
    enum NewFileError {
        AccountRetrievalError(account_repo::Error),
        EncryptedFileError(file_encryption_service::FileCreationError),
        SavingMetadataFailed(file_metadata_repo::Error),
        SavingFileContentsFailed(file_repo::Error),
    }
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
        unimplemented!()
    }

    fn update(db: &Db, id: Uuid, content: &str) -> Result<EncryptedFile, UpdateFileError> {
        unimplemented!()
    }

    fn get(db: &Db, id: Uuid) -> Result<DecryptedValue, Error> {
        unimplemented!()
    }
}
