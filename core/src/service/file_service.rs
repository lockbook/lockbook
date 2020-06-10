use sled::Db;

use crate::error_enum;
use crate::model::client_file_metadata::ClientFileMetadata;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo;
use crate::repo::file_repo::FileRepo;
use crate::service::crypto_service::DecryptedValue;
use crate::service::file_encryption_service;
use crate::service::file_encryption_service::{EncryptedFile, FileEncryptionService};
use serde::export::PhantomData;

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
    fn create(db: &Db, name: &String, path: &String) -> Result<ClientFileMetadata, NewFileError>;
    fn update(db: &Db, id: &String, content: &String) -> Result<EncryptedFile, UpdateFileError>;
    fn get(db: &Db, id: &String) -> Result<DecryptedValue, Error>;
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
    fn create(db: &Db, name: &String, path: &String) -> Result<ClientFileMetadata, NewFileError> {
        info!("Creating new file with name: {} at path {}", name, path);
        let account = AccountDb::get_account(db)?;
        debug!("Account retrieved: {:?}", account);

        let encrypted_file = FileCrypto::new_file(&account)?;
        debug!("Encrypted file created: {:?}", encrypted_file);

        let meta = FileMetadataDb::insert_new_file(&db, &name, &path)?;
        debug!("Metadata for file: {:?}", meta);

        FileDb::update(db, &meta.id, &encrypted_file)?;
        info!("New file saved locally");
        Ok(meta)
    }

    fn update(db: &Db, id: &String, content: &String) -> Result<EncryptedFile, UpdateFileError> {
        info!("Replacing file id: {} contents with: {}", &id, &content);

        let account = AccountDb::get_account(db)?;
        debug!("Account retrieved: {:?}", account);

        let encrypted_file = FileDb::get(db, &id)?;
        debug!("Metadata of the file to edit: {:?}", encrypted_file);

        let updated_enc_file = FileCrypto::write_to_file(
            &account,
            &encrypted_file,
            &DecryptedValue {
                secret: content.clone(),
            },
        )?;
        debug!("New encrypted file: {:?}", updated_enc_file);

        FileDb::update(db, &id, &updated_enc_file)?;

        let meta = FileMetadataDb::get(db, &id)?;
        debug!("New metadata: {:?}", &meta);
        FileMetadataDb::update(
            db,
            &ClientFileMetadata {
                id: id.clone(),
                name: meta.name,
                parent_id: meta.parent_id,
                content_version: meta.content_version,
                metadata_version: meta.metadata_version,
                new: meta.new,
                document_edited: true,
                metadata_changed: false,
                deleted: false,
            },
        )?;
        info!("Updated file {:?} contents {:?}", &id, &content);
        Ok(updated_enc_file)
    }

    fn get(db: &Db, id: &String) -> Result<DecryptedValue, Error> {
        info!("Getting file contents {:?}", &id);
        let account = AccountDb::get_account(db)?;
        let encrypted_file = FileDb::get(db, &id)?;
        let decrypted_file = FileCrypto::read_file(&account, &encrypted_file)?;
        Ok(decrypted_file)
    }
}
