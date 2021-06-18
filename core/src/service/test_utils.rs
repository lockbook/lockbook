#![allow(dead_code)]

use crate::model::state::Config;
use crate::repo::file_metadata_repo::FILE_METADATA;
use crate::repo::local_changes_repo;

use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::{FileMetadata, FileType};

use crate::repo::{
    account_repo, db_version_repo, document_repo, file_metadata_repo, local_storage,
};
use lockbook_crypto::{pubkey, symkey};
use lockbook_models::file_metadata::FileType::Folder;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

#[macro_export]
macro_rules! assert_matches (
    ($actual:expr, $expected:pat) => {
        // Only compute actual once
        let actual_value = $actual;
        match actual_value {
            $expected => {},
            _ => panic!("assertion failed: {:?} did not match expectation", actual_value)
        }
    }
);

#[macro_export]
macro_rules! path {
    ($account:expr, $path:expr) => {{
        &format!("{}/{}", $account.username, $path)
    }};
}

pub fn test_config() -> Config {
    Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4().to_string()),
    }
}

pub fn random_username() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn random_filename() -> SecretFileName {
    let name: String = Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    symkey::encrypt_and_hmac(&symkey::generate_key(), &name).unwrap()
}

pub fn url() -> String {
    env::var("API_URL").expect("API_URL must be defined!")
}

pub fn generate_account() -> Account {
    Account {
        username: random_username(),
        api_url: url(),
        private_key: pubkey::generate_key(),
    }
}

pub fn generate_root_metadata(account: &Account) -> (FileMetadata, AESKey) {
    let id = Uuid::new_v4();
    let key = symkey::generate_key();
    let name = symkey::encrypt_and_hmac(&key, &account.username.clone()).unwrap();
    let key_encryption_key =
        pubkey::get_aes_key(&account.private_key, &account.public_key()).unwrap();
    let encrypted_access_key = symkey::encrypt(&key_encryption_key, &key).unwrap();
    let use_access_key = UserAccessInfo {
        username: account.username.clone(),
        encrypted_by: account.public_key(),
        access_key: encrypted_access_key,
    };

    let mut user_access_keys = HashMap::new();
    user_access_keys.insert(account.username.clone(), use_access_key);

    (
        FileMetadata {
            file_type: Folder,
            id,
            name,
            owner: account.username.clone(),
            parent: id,
            content_version: 0,
            metadata_version: 0,
            deleted: false,
            user_access_keys,
            folder_access_keys: symkey::encrypt(&symkey::generate_key(), &key).unwrap(),
        },
        key,
    )
}

pub fn generate_file_metadata(
    account: &Account,
    parent: &FileMetadata,
    parent_key: &AESKey,
    file_type: FileType,
) -> (FileMetadata, AESKey) {
    let id = Uuid::new_v4();
    let file_key = symkey::generate_key();
    (
        FileMetadata {
            file_type,
            id,
            name: random_filename(),
            owner: account.username.clone(),
            parent: parent.id,
            content_version: 0,
            metadata_version: 0,
            deleted: false,
            user_access_keys: Default::default(),
            folder_access_keys: aes_encrypt(parent_key, &file_key),
        },
        file_key,
    )
}

pub fn aes_encrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey,
    to_encrypt: &T,
) -> AESEncrypted<T> {
    symkey::encrypt(key, to_encrypt).unwrap()
}

pub fn aes_decrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey,
    to_decrypt: &AESEncrypted<T>,
) -> T {
    symkey::decrypt(&key, &to_decrypt).unwrap()
}

pub fn assert_dbs_eq(db1: &Config, db2: &Config) {
    let value1: Vec<FileMetadata> = local_storage::dump::<_, Vec<u8>>(&db1, FILE_METADATA)
        .unwrap()
        .iter()
        .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
        .collect();

    let value2: Vec<FileMetadata> = local_storage::dump::<_, Vec<u8>>(&db2, FILE_METADATA)
        .unwrap()
        .iter()
        .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
        .collect();
    assert_eq!(value1, value2);

    assert_eq!(
        account_repo::get_account(&db1).unwrap(),
        account_repo::get_account(&db2).unwrap()
    );

    assert_eq!(
        local_changes_repo::get_all_local_changes(&db1).unwrap(),
        local_changes_repo::get_all_local_changes(&db2).unwrap()
    );

    assert_eq!(
        db_version_repo::get(&db1).unwrap(),
        db_version_repo::get(&db2).unwrap()
    );

    assert_eq!(
        file_metadata_repo::get_last_updated(&db1).unwrap(),
        file_metadata_repo::get_last_updated(&db2).unwrap()
    );

    let value1: Vec<EncryptedDocument> =
        local_storage::dump::<_, Vec<u8>>(&db1, document_repo::NAMESPACE)
            .unwrap()
            .iter()
            .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
            .collect();
    let value2: Vec<EncryptedDocument> =
        local_storage::dump::<_, Vec<u8>>(&db2, document_repo::NAMESPACE)
            .unwrap()
            .iter()
            .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
            .collect();
    assert_eq!(value1, value2);
}
