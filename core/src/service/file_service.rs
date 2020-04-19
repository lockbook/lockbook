use sled::Db;

use crate::model::file_metadata::{FileMetadata, Status};
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo;
use crate::repo::file_repo::FileRepo;
use crate::service::crypto_service::DecryptedValue;
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::FileEncryptionService;
use crate::{error_enum, info};
use serde::export::PhantomData;

error_enum! {
    enum Error {
        FileRepo(file_repo::Error),
        MetaRepo(file_metadata_repo::Error),
        AccountRepo(account_repo::Error),
        EncryptionServiceWrite(file_encryption_service::FileWriteError),
        EncryptionServiceRead(file_encryption_service::UnableToReadFile),
    }
}

pub trait FileService {
    fn update(db: &Db, id: String, content: String) -> Result<bool, Error>;
    fn get(db: &Db, id: String) -> Result<DecryptedValue, Error>;
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
    fn update(db: &Db, id: String, content: String) -> Result<bool, Error> {
        let account = AccountDb::get_account(db)?;
        let encrypted_file = FileDb::get(db, &id)?;
        let updated_enc_file = FileCrypto::write_to_file(
            &account,
            &encrypted_file,
            &DecryptedValue {
                secret: content.clone(),
            },
        )?;
        FileDb::update(db, &id, &updated_enc_file)?;
        let meta = FileMetadataDb::get(db, &id)?;
        FileMetadataDb::update(
            db,
            &FileMetadata {
                id: id.clone(),
                name: meta.name,
                path: meta.path,
                updated_at: meta.updated_at,
                version: meta.version,
                status: if meta.status == Status::New {
                    Status::New
                } else {
                    Status::Local
                },
            },
        )?;
        info(format!("Updated file {:?} contents {:?}", &id, &content));
        Ok(true)
    }

    fn get(db: &Db, id: String) -> Result<DecryptedValue, Error> {
        info(format!("Getting file contents {:?}", &id));
        let account = AccountDb::get_account(db)?;
        let encrypted_file = FileDb::get(db, &id)?;
        let decrypted_file = FileCrypto::read_file(&account, encrypted_file)?;
        Ok(decrypted_file)
    }
}
