#![allow(dead_code)]

use lockbook_core::model::account::Account;
use lockbook_core::model::crypto::*;
use lockbook_core::model::file_metadata::{FileMetadata, FileType};
use lockbook_core::model::state::Config;
use lockbook_core::repo::account_repo::AccountRepo;
use lockbook_core::repo::db_version_repo::DbVersionRepo;
use lockbook_core::repo::document_repo::DocumentRepo;
use lockbook_core::repo::file_metadata_repo::{FileMetadataRepo, FILE_METADATA};
use lockbook_core::repo::local_changes_repo::LocalChangesRepo;
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{
    AESImpl, PubKeyCryptoService, RSAImpl, SymmetricCryptoService,
};
use lockbook_core::storage::db_provider::Backend;
use lockbook_core::{
    DefaultAccountRepo, DefaultBackend, DefaultDbVersionRepo, DefaultDocumentRepo,
    DefaultFileMetadataRepo, DefaultLocalChangesRepo,
};
use rsa::{RSAPrivateKey, RSAPublicKey};
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

pub fn test_db() -> <DefaultBackend as Backend>::Db {
    <DefaultBackend as Backend>::connect_to_db(&test_config()).unwrap()
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

pub fn random_filename() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn generate_account() -> Account {
    Account {
        username: random_username(),
        api_url: env::var("API_URL").expect("API_URL must be defined!"),
        private_key: RSAImpl::<ClockImpl>::generate_key().unwrap(),
    }
}

pub fn generate_root_metadata(account: &Account) -> (FileMetadata, AESKey) {
    let id = Uuid::new_v4();
    let folder_key = AESImpl::generate_key();

    let public_key = account.private_key.to_public_key();
    let user_access_info = UserAccessInfo {
        username: account.username.clone(),
        public_key: public_key.clone(),
        access_key: rsa_encrypt(&public_key, &folder_key),
    };
    let mut user_access_keys = HashMap::new();
    user_access_keys.insert(account.username.clone(), user_access_info);

    (
        FileMetadata {
            file_type: FileType::Folder,
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
                access_key: aes_encrypt(&folder_key, &folder_key),
            },
        },
        folder_key,
    )
}

pub fn generate_file_metadata(
    account: &Account,
    parent: &FileMetadata,
    parent_key: &AESKey,
    file_type: FileType,
) -> (FileMetadata, AESKey) {
    let id = Uuid::new_v4();
    let file_key = AESImpl::generate_key();
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
            folder_access_keys: FolderAccessInfo {
                folder_id: id,
                access_key: aes_encrypt(parent_key, &file_key),
            },
        },
        file_key,
    )
}

pub fn aes_encrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey,
    to_encrypt: &T,
) -> AESEncrypted<T> {
    AESImpl::encrypt(key, to_encrypt).unwrap()
}

pub fn aes_decrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey,
    to_decrypt: &AESEncrypted<T>,
) -> T {
    AESImpl::decrypt(&key, &to_decrypt).unwrap()
}

pub fn rsa_encrypt<T: Serialize + DeserializeOwned>(
    key: &RSAPublicKey,
    to_encrypt: &T,
) -> RSAEncrypted<T> {
    RSAImpl::<ClockImpl>::encrypt(key, to_encrypt).unwrap()
}

pub fn rsa_decrypt<T: Serialize + DeserializeOwned>(
    key: &RSAPrivateKey,
    to_decrypt: &RSAEncrypted<T>,
) -> T {
    RSAImpl::<ClockImpl>::decrypt(key, to_decrypt).unwrap()
}

pub fn assert_dbs_eq(db1: &<DefaultBackend as Backend>::Db, db2: &<DefaultBackend as Backend>::Db) {
    let value1: Vec<FileMetadata> = DefaultBackend::dump::<_, Vec<u8>>(&db1, FILE_METADATA)
        .unwrap()
        .iter()
        .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
        .collect();

    let value2: Vec<FileMetadata> = DefaultBackend::dump::<_, Vec<u8>>(&db2, FILE_METADATA)
        .unwrap()
        .iter()
        .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
        .collect();
    assert_eq!(value1, value2);

    assert_eq!(
        DefaultAccountRepo::get_account(&db1).unwrap(),
        DefaultAccountRepo::get_account(&db2).unwrap()
    );

    assert_eq!(
        DefaultLocalChangesRepo::get_all_local_changes(&db1).unwrap(),
        DefaultLocalChangesRepo::get_all_local_changes(&db2).unwrap()
    );

    assert_eq!(
        DefaultDbVersionRepo::get(&db1).unwrap(),
        DefaultDbVersionRepo::get(&db2).unwrap()
    );

    assert_eq!(
        DefaultFileMetadataRepo::get_last_updated(&db1).unwrap(),
        DefaultFileMetadataRepo::get_last_updated(&db2).unwrap()
    );

    let value1: Vec<EncryptedDocument> =
        DefaultBackend::dump::<_, Vec<u8>>(&db1, DefaultDocumentRepo::NAMESPACE)
            .unwrap()
            .iter()
            .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
            .collect();
    let value2: Vec<EncryptedDocument> =
        DefaultBackend::dump::<_, Vec<u8>>(&db2, DefaultDocumentRepo::NAMESPACE)
            .unwrap()
            .iter()
            .map(|s| serde_json::from_slice(s.as_ref()).unwrap())
            .collect();
    assert_eq!(value1, value2);
}
