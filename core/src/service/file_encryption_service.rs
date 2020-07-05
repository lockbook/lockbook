use std::collections::HashMap;

use serde::export::PhantomData;
use uuid::Uuid;

use crate::model::account::Account;
use crate::model::file_metadata::FileType::Folder;
use crate::model::file_metadata::{ClientFileMetadata, FileType};
use crate::model::crypto::*;
use crate::service::crypto_service::{
    AesDecryptionFailed, AesEncryptionFailed, DecryptionFailed, PubKeyCryptoService,
    SymmetricCryptoService,
};

#[derive(Debug)]
pub enum KeyDecryptionFailure {
    ClientMetadataMissing(()),
    AesDecryptionFailed(AesDecryptionFailed),
    PKDecryptionFailed(DecryptionFailed),
}

#[derive(Debug)]
pub enum RootFolderCreationError {
    FailedToPKEncryptAccessKey(rsa::errors::Error),
    FailedToAesEncryptAccessKey(AesEncryptionFailed),
}

#[derive(Debug)]
pub enum FileCreationError {
    ParentKeyDecryptionFailed(KeyDecryptionFailure),
    AesEncryptionFailed(AesEncryptionFailed),
}

#[derive(Debug)]
pub enum FileWriteError {
    FileKeyDecryptionFailed(KeyDecryptionFailure),
    AesEncryptionFailed(AesEncryptionFailed),
}

#[derive(Debug)]
pub enum UnableToReadFile {
    FileKeyDecryptionFailed(KeyDecryptionFailure),
    AesDecryptionFailed(AesDecryptionFailed),
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

    fn write_to_document(
        // TODO add checks for folders?
        account: &Account,
        content: &DecryptedValue,
        metadata: &ClientFileMetadata,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<Document, FileWriteError>;

    fn read_document(
        // TODO add checks for folders?
        account: &Account,
        file: &Document,
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
        let access_key = parents
            .get(&id)
            .ok_or(())
            .map_err(KeyDecryptionFailure::ClientMetadataMissing)?;
        match access_key.user_access_keys.get(&account.username) {
            None => {
                let folder_access = access_key.folder_access_keys.clone();

                let decrypted_parent =
                    Self::decrypt_key_for_file(account, folder_access.folder_id, parents)?;

                let key = AES::decrypt(&decrypted_parent, &folder_access.access_key)
                    .map_err(KeyDecryptionFailure::AesDecryptionFailed)?
                    .secret;

                Ok(AesKey { key })
            }
            Some(user_access) => {
                let key = PK::decrypt(&account.keys, &user_access.access_key)
                    .map_err(KeyDecryptionFailure::PKDecryptionFailed)?
                    .secret;
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
        let parent_key = Self::decrypt_key_for_file(&account, parent_id, parents)
            .map_err(FileCreationError::ParentKeyDecryptionFailed)?;
        let access_key = AES::encrypt(&parent_key, &DecryptedValue { secret })
            .map_err(FileCreationError::AesEncryptionFailed)?;
        let id = Uuid::new_v4();

        Ok(ClientFileMetadata {
            file_type,
            id,
            name: name.to_string(),
            parent: parent_id,
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
        )
        .map_err(RootFolderCreationError::FailedToPKEncryptAccessKey)?;
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
            parent: id,
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
                )
                .map_err(RootFolderCreationError::FailedToAesEncryptAccessKey)?,
            },
        })
    }

    fn write_to_document(
        account: &Account,
        content: &DecryptedValue,
        metadata: &ClientFileMetadata,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<Document, FileWriteError> {
        let key = Self::decrypt_key_for_file(&account, metadata.id, parents)
            .map_err(FileWriteError::FileKeyDecryptionFailed)?;

        Ok(Document {
            content: AES::encrypt(&key, &content).map_err(FileWriteError::AesEncryptionFailed)?,
        })
    }

    fn read_document(
        account: &Account,
        file: &Document,
        metadata: &ClientFileMetadata,
        parents: HashMap<Uuid, ClientFileMetadata>,
    ) -> Result<DecryptedValue, UnableToReadFile> {
        let key = Self::decrypt_key_for_file(&account, metadata.id, parents)
            .map_err(UnableToReadFile::FileKeyDecryptionFailed)?;

        Ok(AES::decrypt(&key, &file.content).map_err(UnableToReadFile::AesDecryptionFailed)?)
    }
}

#[cfg(test)]
mod unit_tests {
    use std::collections::HashMap;

    use crate::model::account::Account;
    use crate::model::file_metadata::FileType::{Document, Folder};
    use crate::model::crypto::DecryptedValue;
    use crate::service::crypto_service::PubKeyCryptoService;
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::{DefaultCrypto, DefaultFileEncryptionService};

    #[test]
    fn test_root_folder() {
        let keys = DefaultCrypto::generate_key().unwrap();

        let account = Account {
            username: String::from("username"),
            keys,
        };

        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        assert_eq!(root.id, root.parent);
        assert_eq!(root.file_type, Folder);
        assert!(root.user_access_keys.contains_key("username"));
        assert_eq!(root.folder_access_keys.folder_id, root.id);

        let mut parents = HashMap::new();

        parents.insert(root.id, root.clone());

        let sub_child = DefaultFileEncryptionService::create_file_metadata(
            "test_folder1",
            Folder,
            root.id,
            &account,
            parents.clone(),
        )
        .unwrap();
        parents.insert(sub_child.id, sub_child.clone());

        let sub_sub_child = DefaultFileEncryptionService::create_file_metadata(
            "test_folder2",
            Folder,
            sub_child.id,
            &account,
            parents.clone(),
        )
        .unwrap();
        parents.insert(sub_sub_child.id, sub_sub_child.clone());

        let deep_file = DefaultFileEncryptionService::create_file_metadata(
            "file",
            Document,
            sub_sub_child.id,
            &account,
            parents.clone(),
        )
        .unwrap();
        parents.insert(deep_file.id, deep_file.clone());

        let public_content = DefaultFileEncryptionService::write_to_document(
            &account,
            &DecryptedValue {
                secret: "test content".to_string(),
            },
            &deep_file,
            parents.clone(),
        )
        .unwrap();

        let private_content = DefaultFileEncryptionService::read_document(
            &account,
            &public_content,
            &deep_file,
            parents.clone(),
        )
        .unwrap();

        assert_eq!(private_content.secret, "test content");
    }
}
