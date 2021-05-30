use lockbook_crypto::pubkey::GetAesKeyError;
use lockbook_crypto::symkey::AESDecryptError;
use lockbook_crypto::symkey::AESEncryptError;
use std::collections::HashMap;

use uuid::Uuid;

use crate::service::file_encryption_service::UnableToGetKeyForUser::UnableToDecryptKey;
use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::FileType::Folder;
use lockbook_models::file_metadata::{FileMetadata, FileType};
use std::collections::hash_map::RandomState;

#[derive(Debug)]
pub enum KeyDecryptionFailure {
    ClientMetadataMissing(()),
    SharedSecretError(GetAesKeyError),
    KeyDecryptionError(AESDecryptError),
}

pub fn decrypt_key_for_file(
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

            let decrypted_parent = decrypt_key_for_file(account, access_key.parent, parents)?;

            let key = symkey::decrypt(&decrypted_parent, &folder_access)
                .map_err(KeyDecryptionFailure::KeyDecryptionError)?;

            Ok(key)
        }
        Some(user_access) => {
            let access_key_key =
                pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by)
                    .map_err(KeyDecryptionFailure::SharedSecretError)?;
            let key = symkey::decrypt(&access_key_key, &user_access.access_key)
                .map_err(KeyDecryptionFailure::KeyDecryptionError)?;
            Ok(key)
        }
    }
}

#[derive(Debug)]
pub enum FileCreationError {
    ParentKeyDecryptionFailed(KeyDecryptionFailure),
    AesEncryptionFailed(AESEncryptError),
}

pub fn re_encrypt_key_for_file(
    personal_key: &Account,
    file_key: AESKey,
    new_parent_id: Uuid,
    parents: HashMap<Uuid, FileMetadata>,
) -> Result<EncryptedFolderAccessKey, FileCreationError> {
    let parent_key = decrypt_key_for_file(&personal_key, new_parent_id, parents)
        .map_err(FileCreationError::ParentKeyDecryptionFailed)?;

    let access_key =
        symkey::encrypt(&parent_key, &file_key).map_err(FileCreationError::AesEncryptionFailed)?;

    Ok(access_key)
}

#[derive(Debug)]
pub enum UnableToGetKeyForUser {
    UnableToDecryptKey(KeyDecryptionFailure),
    SharedSecretError(GetAesKeyError),
    KeyEncryptionError(AESEncryptError),
}

pub fn get_key_for_user(
    account: &Account,
    id: Uuid,
    parents: HashMap<Uuid, FileMetadata, RandomState>,
) -> Result<UserAccessInfo, UnableToGetKeyForUser> {
    let key = decrypt_key_for_file(&account, id, parents).map_err(UnableToDecryptKey)?;

    let public_key = account.public_key();

    let key_encryption_key = pubkey::get_aes_key(&account.private_key, &account.public_key())
        .map_err(UnableToGetKeyForUser::SharedSecretError)?;

    let access_key = symkey::encrypt(&key_encryption_key, &key)
        .map_err(UnableToGetKeyForUser::KeyEncryptionError)?;

    Ok(UserAccessInfo {
        username: account.username.clone(),
        encrypted_by: public_key,
        access_key,
    })
}

pub fn create_file_metadata(
    name: &str,
    file_type: FileType,
    parent: Uuid,
    account: &Account,
    parents: HashMap<Uuid, FileMetadata>,
) -> Result<FileMetadata, FileCreationError> {
    let parent_key = decrypt_key_for_file(&account, parent, parents)
        .map_err(FileCreationError::ParentKeyDecryptionFailed)?;
    let folder_access_keys = symkey::encrypt(&parent_key, &symkey::generate_key())
        .map_err(FileCreationError::AesEncryptionFailed)?;
    let id = Uuid::new_v4();

    Ok(FileMetadata {
        file_type,
        id,
        name: name.to_string(),
        owner: account.username.to_string(),
        parent,
        content_version: 0,
        metadata_version: 0,
        deleted: false,
        user_access_keys: Default::default(),
        folder_access_keys,
    })
}

#[derive(Debug)]
pub enum RootFolderCreationError {
    FailedToAesEncryptAccessKey(AESEncryptError),
    SharedSecretError(GetAesKeyError),
}

pub fn create_metadata_for_root_folder(
    account: &Account,
) -> Result<FileMetadata, RootFolderCreationError> {
    let id = Uuid::new_v4();
    let key = symkey::generate_key();
    let key_encryption_key = pubkey::get_aes_key(&account.private_key, &account.public_key())
        .map_err(RootFolderCreationError::SharedSecretError)?;
    let encrypted_access_key = symkey::encrypt(&key_encryption_key, &key)
        .map_err(RootFolderCreationError::FailedToAesEncryptAccessKey)?;
    let use_access_key = UserAccessInfo {
        username: account.username.clone(),
        encrypted_by: account.public_key(),
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
        folder_access_keys: symkey::encrypt(&symkey::generate_key(), &key)
            .map_err(RootFolderCreationError::FailedToAesEncryptAccessKey)?,
    })
}

#[derive(Debug)]
pub enum FileWriteError {
    FileKeyDecryptionFailed(KeyDecryptionFailure),
    AesEncryptionFailed(AESEncryptError),
}

pub fn write_to_document(
    account: &Account,
    content: &[u8],
    metadata: &FileMetadata,
    parents: HashMap<Uuid, FileMetadata>,
) -> Result<EncryptedDocument, FileWriteError> {
    let key = decrypt_key_for_file(&account, metadata.id, parents)
        .map_err(FileWriteError::FileKeyDecryptionFailed)?;
    symkey::encrypt(&key, &content.to_vec()).map_err(FileWriteError::AesEncryptionFailed)
}

#[derive(Debug)]
pub enum UnableToReadFile {
    FileKeyDecryptionFailed(KeyDecryptionFailure),
    AesDecryptionFailed(AESDecryptError),
}

pub fn read_document(
    account: &Account,
    file: &EncryptedDocument,
    metadata: &FileMetadata,
    parents: HashMap<Uuid, FileMetadata>,
) -> Result<DecryptedDocument, UnableToReadFile> {
    let key = decrypt_key_for_file(&account, metadata.id, parents)
        .map_err(UnableToReadFile::FileKeyDecryptionFailed)?;
    symkey::decrypt(&key, file).map_err(UnableToReadFile::AesDecryptionFailed)
}

#[derive(Debug)]
pub enum UnableToReadFileAsUser {
    SharedSecretError(GetAesKeyError),
    AesKeyDecryptionFailed(AESDecryptError),
    AesContentDecryptionFailed(AESDecryptError),
}

pub fn user_read_document(
    account: &Account,
    file: &EncryptedDocument,
    user_access_info: &UserAccessInfo,
) -> Result<DecryptedDocument, UnableToReadFileAsUser> {
    let key_decryption_key =
        pubkey::get_aes_key(&account.private_key, &user_access_info.encrypted_by)
            .map_err(UnableToReadFileAsUser::SharedSecretError)?;
    let key = symkey::decrypt(&key_decryption_key, &user_access_info.access_key)
        .map_err(UnableToReadFileAsUser::AesKeyDecryptionFailed)?;

    let content =
        symkey::decrypt(&key, file).map_err(UnableToReadFileAsUser::AesContentDecryptionFailed)?;
    Ok(content)
}

#[cfg(test)]
mod unit_tests {
    use std::collections::HashMap;

    use crate::service::file_encryption_service;
    use lockbook_crypto::pubkey;
    use lockbook_models::account::Account;
    use lockbook_models::file_metadata::FileType::{Document, Folder};

    #[test]
    fn test_root_folder() {
        let keys = pubkey::generate_key();

        let account = Account {
            username: String::from("username"),
            api_url: "ftp://uranus.net".to_string(),
            private_key: keys,
        };

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        assert_eq!(root.id, root.parent);
        assert_eq!(root.file_type, Folder);
        assert!(root.user_access_keys.contains_key("username"));

        let mut parents = HashMap::new();

        parents.insert(root.id, root.clone());

        let sub_child = file_encryption_service::create_file_metadata(
            "test_folder1",
            Folder,
            root.id,
            &account,
            parents.clone(),
        )
        .unwrap();
        parents.insert(sub_child.id, sub_child.clone());

        let sub_sub_child = file_encryption_service::create_file_metadata(
            "test_folder2",
            Folder,
            sub_child.id,
            &account,
            parents.clone(),
        )
        .unwrap();
        parents.insert(sub_sub_child.id, sub_sub_child.clone());

        let deep_file = file_encryption_service::create_file_metadata(
            "file",
            Document,
            sub_sub_child.id,
            &account,
            parents.clone(),
        )
        .unwrap();
        parents.insert(deep_file.id, deep_file.clone());

        let public_content = file_encryption_service::write_to_document(
            &account,
            "test content".as_bytes(),
            &deep_file,
            parents.clone(),
        )
        .unwrap();

        let private_content = file_encryption_service::read_document(
            &account,
            &public_content,
            &deep_file,
            parents.clone(),
        )
        .unwrap();

        assert_eq!(private_content, "test content".as_bytes());
    }
}
