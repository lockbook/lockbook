use sled::Db;

use crate::error_enum;
use crate::model::client_file_metadata::ClientFileMetadata;
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
    fn create(db: &Db, name: &str, parent: Uuid) -> Result<ClientFileMetadata, NewFileError>;
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
    fn create(db: &Db, name: &str, parent: Uuid) -> Result<ClientFileMetadata, NewFileError> {
        info!(
            "Creating new file with name: {} with parent {}",
            name, parent
        );
        let account = AccountDb::get_account(db)?;
        debug!("Account retrieved: {:?}", account);

        let encrypted_file = FileCrypto::new_file(&account)?;
        debug!("Encrypted file created: {:?}", encrypted_file);

        let meta = FileMetadataDb::insert_new_file(&db, &name, parent)?;
        debug!("Metadata for file: {:?}", meta);

        FileDb::update(db, meta.id, &encrypted_file)?;
        info!("New file saved locally");
        Ok(meta)
    }

    fn update(db: &Db, id: Uuid, content: &str) -> Result<EncryptedFile, UpdateFileError> {
        info!("Replacing file id: {} contents with: {}", id, content);

        let account = AccountDb::get_account(db)?;
        debug!("Account retrieved: {:?}", account);

        let encrypted_file = FileDb::get(db, id)?;
        debug!("Metadata of the file to edit: {:?}", encrypted_file);

        let updated_enc_file = FileCrypto::write_to_file(
            &account,
            &encrypted_file,
            &DecryptedValue {
                secret: String::from(content),
            },
        )?;
        debug!("New encrypted file: {:?}", updated_enc_file);

        FileDb::update(db, id, &updated_enc_file)?;

        let meta = FileMetadataDb::get(db, id)?;
        debug!("New metadata: {:?}", meta);
        FileMetadataDb::update(
            db,
            &ClientFileMetadata {
                id: id,
                name: meta.name,
                parent_id: meta.parent_id,
                content_version: meta.content_version,
                metadata_version: meta.metadata_version,
                user_access_keys: meta.user_access_keys,
                folder_access_keys: meta.folder_access_keys,
                new: meta.new,
                document_edited: true,
                metadata_changed: false,
                deleted: false,
            },
        )?;
        info!("Updated file {:?} contents {:?}", id, content);
        Ok(updated_enc_file)
    }

    fn get(db: &Db, id: Uuid) -> Result<DecryptedValue, Error> {
        info!("Getting file contents {:?}", id);
        let account = AccountDb::get_account(db)?;
        let encrypted_file = FileDb::get(db, id)?;
        let decrypted_file = FileCrypto::read_file(&account, &encrypted_file)?;
        Ok(decrypted_file)
    }
}
