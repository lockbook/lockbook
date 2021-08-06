use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::{account_repo, file_repo};
use crate::{core_err_unexpected, CoreError};
use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::FileType::Folder;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use std::collections::HashMap;
use uuid::Uuid;

/// Converts a set of DecryptedFileMetadata's to FileMetadata's. All parents of files must be included in files. Sharing is not supported; user access keys are encrypted for the provided account. This is a pure function.
pub fn encrypt_metadata(
    account: &Account,
    files: &[DecryptedFileMetadata],
) -> Result<Vec<FileMetadata>, CoreError> {
    let mut result = Vec::new();
    for target in files {
        let parent_key = files
            .iter()
            .find(|m| m.id == target.parent)
            .ok_or(CoreError::Unexpected(String::from(
                "encrypt_metadata: missing parent metadata",
            )))?
            .decrypted_folder_access_keys;
        result.push(FileMetadata {
            id: target.id,
            file_type: target.file_type,
            parent: target.parent,
            name: encrypt_file_name(&target.decrypted_name, &parent_key)?,
            owner: target.owner.clone(),
            metadata_version: target.metadata_version,
            content_version: target.content_version,
            deleted: target.deleted,
            user_access_keys: encrypt_user_access_keys(account, &target.decrypted_user_access_key)?,
            folder_access_keys: encrypt_folder_access_keys(
                &target.decrypted_folder_access_keys,
                &parent_key,
            )?,
        })
    }
    Ok(result)
}

fn encrypt_file_name(
    decrypted_name: &str,
    parent_key: &AESKey,
) -> Result<SecretFileName, CoreError> {
    symkey::encrypt_and_hmac(parent_key, decrypted_name).map_err(core_err_unexpected)
}

fn encrypt_user_access_keys(
    account: &Account,
    maybe_decrypted_file_key: &Option<AESKey>,
) -> Result<HashMap<String, UserAccessInfo>, CoreError> {
    match maybe_decrypted_file_key {
        Some(decrypted_file_key) => {
            let user_key = pubkey::get_aes_key(&account.private_key, &account.public_key())
                .map_err(core_err_unexpected)?;
            let encrypted_file_key =
                symkey::encrypt(&user_key, decrypted_file_key).map_err(core_err_unexpected)?;
            let mut result = HashMap::new();
            result.insert(
                account.username.clone(),
                UserAccessInfo {
                    username: account.username.clone(),
                    encrypted_by: account.public_key(),
                    access_key: encrypted_file_key,
                },
            );
            Ok(result)
        }
        None => Ok(Default::default()),
    }
}

fn encrypt_folder_access_keys(
    target_key: &AESKey,
    parent_key: &AESKey,
) -> Result<EncryptedFolderAccessKey, CoreError> {
    symkey::encrypt(parent_key, target_key).map_err(core_err_unexpected)
}

/// Converts a set of FileMetadata's to DecryptedFileMetadata's. All parents of files must be included in files. Sharing is not supported; user access keys not for the provided account are ignored. This is a pure function.
/// CPU optimization opportunity: this function decrypts all ancestors for each file provided, which duplicates a lot of decryption.
pub fn decrypt_metadata(
    account: &Account,
    files: &[FileMetadata],
) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
    let mut result = Vec::new();
    for target in files {
        let parent_key = decrypt_file_key(account, target.parent, files)?;
        result.push(DecryptedFileMetadata {
            id: target.id,
            file_type: target.file_type,
            parent: target.parent,
            decrypted_name: decrypt_file_name(&target.name, &parent_key)?,
            owner: target.owner.clone(),
            metadata_version: target.metadata_version,
            content_version: target.content_version,
            deleted: target.deleted,
            decrypted_user_access_key: decrypt_user_access_keys(account, &target.user_access_keys)?,
            decrypted_folder_access_keys: decrypt_folder_access_keys(
                &target.folder_access_keys,
                &parent_key,
            )?,
        })
    }
    Ok(result)
}

fn decrypt_file_key(
    account: &Account,
    target_id: Uuid,
    target_with_ancestors: &[FileMetadata],
) -> Result<AESKey, CoreError> {
    let target = target_with_ancestors
        .iter()
        .find(|m| m.id == target_id)
        .ok_or(CoreError::Unexpected(String::from(
            "client metadata missing",
        )))?;
    match target.user_access_keys.get(&account.username) {
        Some(user_access) => {
            let user_access_key =
                pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by)
                    .map_err(core_err_unexpected)?;
            let key = symkey::decrypt(&user_access_key, &user_access.access_key)
                .map_err(core_err_unexpected)?;
            Ok(key)
        }
        None => {
            let parent_key = decrypt_file_key(&account, target.parent, target_with_ancestors)?;
            let key = symkey::decrypt(&parent_key, &target.folder_access_keys)
                .map_err(core_err_unexpected)?;
            Ok(key)
        }
    }
}

fn decrypt_file_name(
    encrypted_name: &SecretFileName,
    parent_key: &AESKey,
) -> Result<String, CoreError> {
    symkey::decrypt_and_verify(parent_key, encrypted_name).map_err(core_err_unexpected)
}

fn decrypt_user_access_keys(
    account: &Account,
    encrypted_keys: &HashMap<String, UserAccessInfo>,
) -> Result<Option<AESKey>, CoreError> {
    match encrypted_keys.get(&account.username) {
        Some(user_access_info) => {
            let user_access_key =
                pubkey::get_aes_key(&account.private_key, &user_access_info.encrypted_by)
                    .map_err(core_err_unexpected)?;
            let key = symkey::decrypt(&user_access_key, &user_access_info.access_key)
                .map_err(core_err_unexpected)?;
            Ok(Some(key))
        }
        None => Ok(None),
    }
}

fn decrypt_folder_access_keys(
    encrypted_keys: &EncryptedFolderAccessKey,
    parent_key: &AESKey,
) -> Result<AESKey, CoreError> {
    symkey::decrypt(&parent_key, &encrypted_keys).map_err(core_err_unexpected)
}

pub fn decrypt_key_for_file(config: &Config, id: Uuid) -> Result<AESKey, CoreError> {
    let account = account_repo::get(&config)?;
    let parents = file_repo::get_with_ancestors(&config, RepoSource::Local, id)?;
    let access_key = parents
        .get(&id)
        .ok_or(())
        .map_err(|_| CoreError::Unexpected(String::from("client metadata missing")))?;
    match access_key.user_access_keys.get(&account.username) {
        None => {
            let folder_access = access_key.folder_access_keys.clone();
            let decrypted_parent = decrypt_key_for_file(&config, access_key.parent)?;
            let key =
                symkey::decrypt(&decrypted_parent, &folder_access).map_err(core_err_unexpected)?;
            Ok(key)
        }
        Some(user_access) => {
            let access_key_key =
                pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by)
                    .map_err(core_err_unexpected)?;
            let key = symkey::decrypt(&access_key_key, &user_access.access_key)
                .map_err(core_err_unexpected)?;
            Ok(key)
        }
    }
}

pub fn re_encrypt_key_for_file(
    config: &Config,
    file_key: AESKey,
    new_parent_id: Uuid,
) -> Result<EncryptedFolderAccessKey, CoreError> {
    let parent_key = decrypt_key_for_file(&config, new_parent_id)?;
    let access_key = symkey::encrypt(&parent_key, &file_key).map_err(core_err_unexpected)?;
    Ok(access_key)
}

pub fn get_key_for_user(config: &Config, id: Uuid) -> Result<UserAccessInfo, CoreError> {
    let account = account_repo::get(&config)?;
    let key = decrypt_key_for_file(&config, id)?;
    let public_key = account.public_key();
    let key_encryption_key = pubkey::get_aes_key(&account.private_key, &account.public_key())
        .map_err(core_err_unexpected)?;
    let access_key = symkey::encrypt(&key_encryption_key, &key).map_err(core_err_unexpected)?;

    Ok(UserAccessInfo {
        username: account.username,
        encrypted_by: public_key,
        access_key,
    })
}

pub fn create_file_metadata(
    config: &Config,
    name: &str,
    file_type: FileType,
    parent: Uuid,
) -> Result<FileMetadata, CoreError> {
    let account = account_repo::get(&config)?;
    let parent_key = decrypt_key_for_file(&config, parent)?;
    let folder_access_keys =
        symkey::encrypt(&parent_key, &symkey::generate_key()).map_err(core_err_unexpected)?;
    let id = Uuid::new_v4();
    let name = symkey::encrypt_and_hmac(&parent_key, name).map_err(core_err_unexpected)?;

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

pub fn create_metadata_for_root_folder(account: &Account) -> Result<FileMetadata, CoreError> {
    let id = Uuid::new_v4();
    let key = symkey::generate_key();
    let name =
        symkey::encrypt_and_hmac(&key, &account.username.clone()).map_err(core_err_unexpected)?;
    let key_encryption_key = pubkey::get_aes_key(&account.private_key, &account.public_key())
        .map_err(core_err_unexpected)?;
    let encrypted_access_key =
        symkey::encrypt(&key_encryption_key, &key).map_err(core_err_unexpected)?;
    let user_access_key = UserAccessInfo {
        username: account.username.clone(),
        encrypted_by: account.public_key(),
        access_key: encrypted_access_key,
    };

    let mut user_access_keys = HashMap::new();
    user_access_keys.insert(account.username.clone(), user_access_key);

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
            .map_err(core_err_unexpected)?,
    })
}

pub fn write_to_document(
    config: &Config,
    content: &[u8],
    metadata: &FileMetadata,
) -> Result<EncryptedDocument, CoreError> {
    let key = decrypt_key_for_file(&config, metadata.id)?;
    symkey::encrypt(&key, &content.to_vec()).map_err(core_err_unexpected)
}

pub fn read_document(
    config: &Config,
    file: &EncryptedDocument,
    metadata: &FileMetadata,
) -> Result<DecryptedDocument, CoreError> {
    let key = decrypt_key_for_file(&config, metadata.id)?;
    symkey::decrypt(&key, file).map_err(core_err_unexpected)
}

pub fn user_read_document(
    account: &Account,
    maybe_file: &Option<EncryptedDocument>,
    user_access_info: &UserAccessInfo,
) -> Result<DecryptedDocument, CoreError> {
    match maybe_file {
        None => Ok(Vec::new()),
        Some(file) => {
            let key_decryption_key =
                pubkey::get_aes_key(&account.private_key, &user_access_info.encrypted_by)
                    .map_err(core_err_unexpected)?;
            let key = symkey::decrypt(&key_decryption_key, &user_access_info.access_key)
                .map_err(core_err_unexpected)?;
            Ok(symkey::decrypt(&key, file).map_err(core_err_unexpected)?)
        }
    }
}

pub fn get_name(config: &Config, meta: &FileMetadata) -> Result<String, CoreError> {
    let parent_access_key = decrypt_key_for_file(&config, meta.parent)?;
    symkey::decrypt_and_verify(&parent_access_key, &meta.name).map_err(core_err_unexpected)
}

pub fn create_name(config: &Config, parent: Uuid, name: &str) -> Result<SecretFileName, CoreError> {
    let parent_key = decrypt_key_for_file(&config, parent)?;
    symkey::encrypt_and_hmac(&parent_key, name).map_err(core_err_unexpected)
}

pub fn rekey_secret_filename(
    config: &Config,
    old_meta: &FileMetadata,
    new_parent: &FileMetadata,
) -> Result<SecretFileName, CoreError> {
    let old_name = get_name(&config, &old_meta)?;
    let new_parent_key = decrypt_key_for_file(&config, new_parent.id)?;
    symkey::encrypt_and_hmac(&new_parent_key, &old_name).map_err(core_err_unexpected)
}
