use lockbook_crypto::pubkey::GetAesKeyError;
use lockbook_crypto::symkey::{AESDecryptError, EncryptAndHmacError};
use lockbook_crypto::symkey::{AESEncryptError, DecryptAndVerifyError};
use std::collections::HashMap;

use uuid::Uuid;

use crate::model::state::Config;
use crate::repo::account_repo::AccountRepoError;
use crate::repo::file_metadata_repo::FindingParentsFailed;
use crate::repo::{account_repo, file_metadata_repo};
use crate::service::file_encryption_service::UnableToGetKeyForUser::UnableToDecryptKey;
use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::FileType::Folder;
use lockbook_models::file_metadata::{FileMetadata, FileType};

#[derive(Debug)]
pub enum KeyDecryptionFailure {
    ClientMetadataMissing(()),
    SharedSecretError(GetAesKeyError),
    KeyDecryptionError(AESDecryptError),
    GettingAccountError(AccountRepoError),
    FindingParentsFailed(FindingParentsFailed),
}

pub fn decrypt_key_for_file(config: &Config, id: Uuid) -> Result<AESKey, KeyDecryptionFailure> {
    let account =
        account_repo::get_account(&config).map_err(KeyDecryptionFailure::GettingAccountError)?;
    let parents = file_metadata_repo::get_with_all_parents(&config, id)
        .map_err(KeyDecryptionFailure::FindingParentsFailed)?;
    let access_key = parents
        .get(&id)
        .ok_or(())
        .map_err(KeyDecryptionFailure::ClientMetadataMissing)?;
    match access_key.user_access_keys.get(&account.username) {
        None => {
            let folder_access = access_key.folder_access_keys.clone();

            let decrypted_parent = decrypt_key_for_file(&config, access_key.parent)?;

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

pub fn re_encrypt_key_for_file(
    config: &Config,
    file_key: AESKey,
    new_parent_id: Uuid,
) -> Result<EncryptedFolderAccessKey, FileCreationError> {
    let parent_key = decrypt_key_for_file(&config, new_parent_id)
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
    AccountRepoError(AccountRepoError),
}

pub fn get_key_for_user(
    config: &Config,
    id: Uuid,
) -> Result<UserAccessInfo, UnableToGetKeyForUser> {
    let account =
        account_repo::get_account(&config).map_err(UnableToGetKeyForUser::AccountRepoError)?;
    let key = decrypt_key_for_file(&config, id).map_err(UnableToDecryptKey)?;

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

#[derive(Debug)]
pub enum FileCreationError {
    ParentKeyDecryptionFailed(KeyDecryptionFailure),
    AesEncryptionFailed(AESEncryptError),
    FileNameCreationError(EncryptAndHmacError),
    GettingAccountError(AccountRepoError),
}

pub fn create_file_metadata(
    config: &Config,
    name: &str,
    file_type: FileType,
    parent: Uuid,
) -> Result<FileMetadata, FileCreationError> {
    let account =
        account_repo::get_account(&config).map_err(FileCreationError::GettingAccountError)?;
    let parent_key = decrypt_key_for_file(&config, parent)
        .map_err(FileCreationError::ParentKeyDecryptionFailed)?;
    let folder_access_keys = symkey::encrypt(&parent_key, &symkey::generate_key())
        .map_err(FileCreationError::AesEncryptionFailed)?;
    let id = Uuid::new_v4();

    let name = symkey::encrypt_and_hmac(&parent_key, name)
        .map_err(FileCreationError::FileNameCreationError)?;

    Ok(FileMetadata {
        file_type,
        id,
        name,
        owner: account.username,
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
    FileNameCreationError(EncryptAndHmacError),
}

pub fn create_metadata_for_root_folder(
    account: &Account,
) -> Result<FileMetadata, RootFolderCreationError> {
    let id = Uuid::new_v4();
    let key = symkey::generate_key();
    let name = symkey::encrypt_and_hmac(&key, &account.username.clone())
        .map_err(RootFolderCreationError::FileNameCreationError)?;
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
        name,
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
    config: &Config,
    content: &[u8],
    metadata: &FileMetadata,
) -> Result<EncryptedDocument, FileWriteError> {
    let key = decrypt_key_for_file(&config, metadata.id)
        .map_err(FileWriteError::FileKeyDecryptionFailed)?;
    symkey::encrypt(&key, &content.to_vec()).map_err(FileWriteError::AesEncryptionFailed)
}

#[derive(Debug)]
pub enum UnableToReadFile {
    FileKeyDecryptionFailed(KeyDecryptionFailure),
    AesDecryptionFailed(AESDecryptError),
}

pub fn read_document(
    config: &Config,
    file: &EncryptedDocument,
    metadata: &FileMetadata,
) -> Result<DecryptedDocument, UnableToReadFile> {
    let key = decrypt_key_for_file(&config, metadata.id)
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

#[derive(Debug)]
pub enum GetNameOfFileError {
    KeyDecryptionFailure(KeyDecryptionFailure),
    DecryptAndVerifyError(DecryptAndVerifyError),
}

pub fn get_name(config: &Config, meta: &FileMetadata) -> Result<String, GetNameOfFileError> {
    let parent_access_key = decrypt_key_for_file(&config, meta.parent)
        .map_err(GetNameOfFileError::KeyDecryptionFailure)?;

    symkey::decrypt_and_verify(&parent_access_key, &meta.name)
        .map_err(GetNameOfFileError::DecryptAndVerifyError)
}

#[derive(Debug)]
pub enum CreateNameError {
    ParentKeyFailure(KeyDecryptionFailure),
    NameEncryptionFailure(EncryptAndHmacError),
}

pub fn create_name(
    config: &Config,
    meta: &FileMetadata,
    name: &str,
) -> Result<SecretFileName, CreateNameError> {
    let parent_key =
        decrypt_key_for_file(&config, meta.parent).map_err(CreateNameError::ParentKeyFailure)?;

    symkey::encrypt_and_hmac(&parent_key, name).map_err(CreateNameError::NameEncryptionFailure)
}
