use crate::model::state::Config;
use crate::repo::{account_repo, file_repo};
use crate::{core_err_unexpected, CoreError};
use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::FileType::Folder;
use lockbook_models::file_metadata::{FileMetadata, FileType};
use std::collections::HashMap;
use uuid::Uuid;

pub fn decrypt_key_for_file(config: &Config, id: Uuid) -> Result<AESKey, CoreError> {
    let account = account_repo::get(&config)?;
    let parents = file_repo::get_with_ancestors(&config, id)?;
    let (access_key, _) = parents
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
    file: &EncryptedDocument,
    user_access_info: &UserAccessInfo,
) -> Result<DecryptedDocument, CoreError> {
    let key_decryption_key =
        pubkey::get_aes_key(&account.private_key, &user_access_info.encrypted_by)
            .map_err(core_err_unexpected)?;
    let key = symkey::decrypt(&key_decryption_key, &user_access_info.access_key)
        .map_err(core_err_unexpected)?;

    let content = symkey::decrypt(&key, file).map_err(core_err_unexpected)?;
    Ok(content)
}

pub fn get_name(config: &Config, meta: &FileMetadata) -> Result<String, CoreError> {
    let parent_access_key = decrypt_key_for_file(&config, meta.parent)?;
    symkey::decrypt_and_verify(&parent_access_key, &meta.name).map_err(core_err_unexpected)
}

pub fn create_name(
    config: &Config,
    meta: &FileMetadata,
    name: &str,
) -> Result<SecretFileName, CoreError> {
    let parent_key = decrypt_key_for_file(&config, meta.parent)?;
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
