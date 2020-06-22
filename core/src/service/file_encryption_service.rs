use serde::export::PhantomData;
use std::collections::HashMap;

use crate::error_enum;
use crate::model::account::Account;
use crate::model::client_file_metadata::FileType::Folder;
use crate::model::client_file_metadata::{ClientFileMetadata, FileType};
use crate::model::crypto::*;
use crate::service::crypto_service::{
    AesDecryptionFailed, AesEncryptionFailed, DecryptionFailed, PubKeyCryptoService,
    SymmetricCryptoService,
};
use uuid::Uuid;

error_enum! {
    enum KeyDecryptionFailure {
        ClientMetadataMissing(()),
        AesDecryptionFailed(AesDecryptionFailed),
        PKDecryptionFailed(DecryptionFailed),
    }
}
error_enum! {
    enum RootFolderCreationError {
        FailedToPKEncryptAccessKey(rsa::errors::Error),
        FailedToAesEncryptAccessKey(AesEncryptionFailed)
    }
}

error_enum! {
    enum FileCreationError {
        ParentKeyDecryptionFailed(KeyDecryptionFailure),
        AesEncryptionFailed(AesEncryptionFailed),
    }
}

error_enum! {
    enum FileWriteError {
        FileKeyDecryptionFailed(KeyDecryptionFailure),
        AesEncryptionFailed(AesEncryptionFailed),
    }
}

error_enum! {
    enum UnableToReadFile {
        FileKeyDecryptionFailed(KeyDecryptionFailure),
        AesDecryptionFailed(AesDecryptionFailed),

    }
}

pub trait FileEncryptionService {
    fn decrypt_key_for_file(
        keys: &Account,
        id: Uuid,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<AesKey, KeyDecryptionFailure>;

    fn create_file_metadata(
        name: &str,
        file_type: FileType,
        parent: Uuid,
        account: &Account,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<ClientFileMetadata, FileCreationError>;

    fn create_metadata_for_root_folder(
        account: &Account,
    ) -> Result<ClientFileMetadata, RootFolderCreationError>;

    fn write_to_file(
        account: &Account,
        content: &DecryptedValue,
        metadata: &ClientFileMetadata,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<EncryptedFile, FileWriteError>;

    fn read_file(
        account: &Account,
        file: &EncryptedFile,
        metadata: &ClientFileMetadata,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<DecryptedValue, UnableToReadFile>;
}

pub struct FileEncryptionServiceImpl<PK: PubKeyCryptoService, AES: SymmetricCryptoService> {
    pk: PhantomData<PK>,
    aes: PhantomData<AES>,
}

impl<PK: PubKeyCryptoService, AES: SymmetricCryptoService> FileEncryptionService
for FileEncryptionServiceImpl<PK, AES>
{
    fn decrypt_key_for_file(
        account: &Account,
        id: Uuid,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<AesKey, KeyDecryptionFailure> {
        let access_key = parents.get(&id).ok_or(())?;
        match access_key.user_access_keys.get(&account.username) {
            None => {
                let folder_access = access_key.folder_access_keys.clone();

                let decrypted_parent =
                    Self::decrypt_key_for_file(account, folder_access.folder_id, parents)?;

                let key = AES::decrypt(&decrypted_parent, &folder_access.access_key)?.secret;

                Ok(AesKey { key })
            }
            Some(user_access) => {
                let key = PK::decrypt(&account.keys, &user_access.access_key)?.secret;
                Ok(AesKey { key })
            }
        }
    }

    fn create_file_metadata(
        name: &str,
        file_type: FileType,
        parent_id: Uuid,
        account: &Account,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<ClientFileMetadata, FileCreationError> {
        let secret = AES::generate_key().key;
        let parent_key = Self::decrypt_key_for_file(&account, parent_id, parents)?;
        let access_key = AES::encrypt(&parent_key, &DecryptedValue { secret })?;
        let id = Uuid::new_v4();

        Ok(ClientFileMetadata {
            file_type,
            id,
            name: name.to_string(),
            parent_id,
            content_version: 0,
            metadata_version: 0,
            new: true,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: parent_id,
                access_key,
            },
        })
    }

    fn create_metadata_for_root_folder(
        account: &Account,
    ) -> Result<ClientFileMetadata, RootFolderCreationError> {
        let id = Uuid::new_v4();
        let public_key = account.keys.to_public_key();
        let key = AES::generate_key();
        let encrypted_access_key = PK::encrypt(
            &public_key,
            &DecryptedValue {
                secret: key.key.clone(),
            },
        )?;
        let use_access_key = UserAccessInfo {
            username: account.username.clone(),
            public_key,
            access_key: encrypted_access_key,
        };

        let mut user_access_keys = HashMap::new();
        user_access_keys.insert(account.username.clone(), use_access_key);

        Ok(ClientFileMetadata {
            file_type: Folder,
            id,
            name: account.username.clone(),
            parent_id: id,
            content_version: 0,
            metadata_version: 0,
            new: false,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
            user_access_keys,
            folder_access_keys: FolderAccessInfo {
                folder_id: id,
                access_key: AES::encrypt(
                    &key,
                    &DecryptedValue {
                        secret: key.key.clone(),
                    },
                )?,
            },
        })
    }

    fn write_to_file(
        account: &Account,
        content: &DecryptedValue,
        metadata: &ClientFileMetadata,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<EncryptedFile, FileWriteError> {
        let key = Self::decrypt_key_for_file(&account, metadata.id, parents)?;

        Ok(EncryptedFile {
            content: AES::encrypt(&key, &content)?,
        })
    }

    fn read_file(
        account: &Account,
        file: &EncryptedFile,
        metadata: &ClientFileMetadata,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<DecryptedValue, UnableToReadFile> {
        let key = Self::decrypt_key_for_file(&account, metadata.id, parents)?;

        Ok(
            AES::decrypt(&key, &file.content)?
        )
    }
}
