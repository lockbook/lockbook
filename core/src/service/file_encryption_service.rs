use std::collections::HashMap;

use uuid::Uuid;

use crate::model::account::Account;
use crate::model::crypto::*;
use crate::model::file_metadata::FileType::Folder;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::service::crypto_service::{
    AESDecryptError, AESEncryptError, PubKeyCryptoService, RSADecryptError, RSAEncryptError,
    SymmetricCryptoService,
};
use crate::service::file_encryption_service::UnableToGetKeyForUser::UnableToDecryptKey;
use std::collections::hash_map::RandomState;

#[derive(Debug)]
pub enum KeyDecryptionFailure {
    ClientMetadataMissing(()),
    AesDecryptionFailed(AESDecryptError),
    PKDecryptionFailed(RSADecryptError),
}

#[derive(Debug)]
pub enum RootFolderCreationError {
    FailedToPKEncryptAccessKey(RSAEncryptError),
    FailedToAesEncryptAccessKey(AESEncryptError),
}

#[derive(Debug)]
pub enum FileCreationError {
    ParentKeyDecryptionFailed(KeyDecryptionFailure),
    AesEncryptionFailed(AESEncryptError),
}

#[derive(Debug)]
pub enum FileWriteError {
    FileKeyDecryptionFailed(KeyDecryptionFailure),
    AesEncryptionFailed(AESEncryptError),
}

#[derive(Debug)]
pub enum UnableToReadFile {
    FileKeyDecryptionFailed(KeyDecryptionFailure),
    AesDecryptionFailed(AESDecryptError),
}

#[derive(Debug)]
pub enum UnableToReadFileAsUser {
    FileKeyDecryptionFailed(RSADecryptError),
    AesDecryptionFailed(AESDecryptError),
}

#[derive(Debug)]
pub enum UnableToGetKeyForUser {
    UnableToDecryptKey(KeyDecryptionFailure),
    FailedToPKEncryptAccessKey(RSAEncryptError),
}

pub trait FileEncryptionService {
    fn decrypt_key_for_file(
        keys: &Account,
        id: Uuid,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<AESKey, KeyDecryptionFailure>;

    fn re_encrypt_key_for_file(
        personal_key: &Account,
        file_key: AESKey,
        new_parent_id: Uuid,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<FolderAccessInfo, FileCreationError>;

    fn get_key_for_user(
        key: &Account,
        id: Uuid,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<UserAccessInfo, UnableToGetKeyForUser>;

    fn create_file_metadata(
        name: &str,
        file_type: FileType,
        parent: Uuid,
        account: &Account,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<FileMetadata, FileCreationError>;

    fn create_metadata_for_root_folder(
        account: &Account,
    ) -> Result<FileMetadata, RootFolderCreationError>;

    fn write_to_document(
        // TODO add checks for folders?
        account: &Account,
        content: &[u8],
        metadata: &FileMetadata,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<EncryptedDocument, FileWriteError>;

    fn read_document(
        account: &Account,
        file: &EncryptedDocument,
        metadata: &FileMetadata,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<DecryptedDocument, UnableToReadFile>;

    fn user_read_document(
        account: &Account,
        file: &EncryptedDocument,
        user_access_info: &UserAccessInfo,
    ) -> Result<DecryptedDocument, UnableToReadFileAsUser>;
}

pub struct FileEncryptionServiceImpl<PK: PubKeyCryptoService, AES: SymmetricCryptoService> {
    _pk: PK,
    _aes: AES,
}

impl<PK: PubKeyCryptoService, AES: SymmetricCryptoService> FileEncryptionService
    for FileEncryptionServiceImpl<PK, AES>
{
    fn decrypt_key_for_file(
        account: &Account,
        id: Uuid,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<AESKey, KeyDecryptionFailure> {
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
                    .map_err(KeyDecryptionFailure::AesDecryptionFailed)?;

                Ok(key)
            }
            Some(user_access) => {
                let key = PK::decrypt(&account.private_key, &user_access.access_key)
                    .map_err(KeyDecryptionFailure::PKDecryptionFailed)?;
                Ok(key)
            }
        }
    }

    fn re_encrypt_key_for_file(
        personal_key: &Account,
        file_key: AESKey,
        new_parent_id: Uuid,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<FolderAccessInfo, FileCreationError> {
        let parent_key = Self::decrypt_key_for_file(&personal_key, new_parent_id, parents)
            .map_err(FileCreationError::ParentKeyDecryptionFailed)?;

        let access_key =
            AES::encrypt(&parent_key, &file_key).map_err(FileCreationError::AesEncryptionFailed)?;

        Ok(FolderAccessInfo {
            folder_id: new_parent_id,
            access_key,
        })
    }

    fn get_key_for_user(
        account: &Account,
        id: Uuid,
        parents: HashMap<Uuid, FileMetadata, RandomState>,
    ) -> Result<UserAccessInfo, UnableToGetKeyForUser> {
        let key = Self::decrypt_key_for_file(&account, id, parents).map_err(UnableToDecryptKey)?;

        let public_key = account.private_key.to_public_key();

        let access_key = PK::encrypt(&public_key, &key)
            .map_err(UnableToGetKeyForUser::FailedToPKEncryptAccessKey)?;

        Ok(UserAccessInfo {
            username: account.username.clone(),
            public_key,
            access_key,
        })
    }

    fn create_file_metadata(
        name: &str,
        file_type: FileType,
        parent_id: Uuid,
        account: &Account,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<FileMetadata, FileCreationError> {
        let parent_key = Self::decrypt_key_for_file(&account, parent_id, parents)
            .map_err(FileCreationError::ParentKeyDecryptionFailed)?;
        let access_key = AES::encrypt(&parent_key, &AES::generate_key())
            .map_err(FileCreationError::AesEncryptionFailed)?;
        let id = Uuid::new_v4();

        Ok(FileMetadata {
            file_type,
            id,
            name: name.to_string(),
            owner: account.username.to_string(),
            parent: parent_id,
            content_version: 0,
            metadata_version: 0,
            deleted: false,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: parent_id,
                access_key,
            },
        }) // TODO: sign
    }

    fn create_metadata_for_root_folder(
        account: &Account,
    ) -> Result<FileMetadata, RootFolderCreationError> {
        let id = Uuid::new_v4();
        let public_key = account.private_key.to_public_key();
        let key = AES::generate_key();
        let encrypted_access_key = PK::encrypt(&public_key, &key)
            .map_err(RootFolderCreationError::FailedToPKEncryptAccessKey)?;
        let use_access_key = UserAccessInfo {
            username: account.username.clone(),
            public_key,
            access_key: encrypted_access_key,
        };

        let mut user_access_keys = HashMap::new();
        user_access_keys.insert(account.username.clone(), use_access_key);

        Ok(FileMetadata {
            file_type: Folder,
            id,
            name: account.username.clone(),
            owner: account.username.clone(),
            parent: id,
            content_version: 0,
            metadata_version: 0,
            deleted: false,
            user_access_keys,
            folder_access_keys: FolderAccessInfo {
                folder_id: id,
                access_key: AES::encrypt(&AES::generate_key(), &key)
                    .map_err(RootFolderCreationError::FailedToAesEncryptAccessKey)?,
            },
        }) // TODO: sign
    }

    fn write_to_document(
        account: &Account,
        content: &[u8],
        metadata: &FileMetadata,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<EncryptedDocument, FileWriteError> {
        let key = Self::decrypt_key_for_file(&account, metadata.id, parents)
            .map_err(FileWriteError::FileKeyDecryptionFailed)?;
        Ok(AES::encrypt(&key, &content.to_vec()).map_err(FileWriteError::AesEncryptionFailed)?)
    }

    fn read_document(
        account: &Account,
        file: &EncryptedDocument,
        metadata: &FileMetadata,
        parents: HashMap<Uuid, FileMetadata>,
    ) -> Result<DecryptedDocument, UnableToReadFile> {
        let key = Self::decrypt_key_for_file(&account, metadata.id, parents)
            .map_err(UnableToReadFile::FileKeyDecryptionFailed)?;
        Ok(AES::decrypt(&key, file).map_err(UnableToReadFile::AesDecryptionFailed)?)
    }

    fn user_read_document(
        account: &Account,
        file: &EncryptedDocument,
        user_access_info: &UserAccessInfo,
    ) -> Result<DecryptedDocument, UnableToReadFileAsUser> {
        let key = PK::decrypt(&account.private_key, &user_access_info.access_key)
            .map_err(UnableToReadFileAsUser::FileKeyDecryptionFailed)?;
        let content =
            AES::decrypt(&key, file).map_err(UnableToReadFileAsUser::AesDecryptionFailed)?;
        Ok(content)
    }
}

#[cfg(test)]
mod unit_tests {
    use std::collections::HashMap;

    use crate::model::account::Account;
    use crate::model::file_metadata::FileType::{Document, Folder};
    use crate::service::crypto_service::PubKeyCryptoService;
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::{DefaultCrypto, DefaultFileEncryptionService};

    #[test]
    fn test_root_folder() {
        let keys = DefaultCrypto::generate_key().unwrap();

        let account = Account {
            username: String::from("username"),
            api_url: "ftp://uranus.net".to_string(),
            private_key: keys,
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
            "test content".as_bytes(),
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

        assert_eq!(private_content, "test content".as_bytes());
    }
}
