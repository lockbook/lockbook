#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::hash::Hash;

use itertools::Itertools;
use lockbook_models::work_unit::WorkUnit;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::FileType::Folder;
use lockbook_models::file_metadata::{DecryptedFileMetadata, EncryptedFileMetadata, FileType};

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::root_repo;
use crate::repo::{account_repo, db_version_repo};
use crate::service::{file_service, integrity_service, path_service, sync_service};

#[macro_export]
macro_rules! assert_dirty_ids {
    ($db:expr, $n:literal) => {
        assert_eq!(
            sync_service::calculate_work(&$db)
                .unwrap()
                .work_units
                .into_iter()
                .map(|wu| wu.get_metadata().id)
                .unique()
                .count(),
            $n
        );
    };
}

pub fn get_dirty_ids(db: &Config, server: bool) -> Vec<Uuid> {
    sync_service::calculate_work(db)
        .unwrap()
        .work_units
        .into_iter()
        .filter_map(|wu| match wu {
            WorkUnit::LocalChange { metadata } if !server => Some(metadata.id),
            WorkUnit::ServerChange { metadata } if server => Some(metadata.id),
            _ => None,
        })
        .unique()
        .collect()
}

pub fn assert_local_work_ids(db: &Config, ids: &[Uuid]) {
    assert!(slices_equal_ignore_order(&get_dirty_ids(db, false), ids));
}

pub fn assert_server_work_ids(db: &Config, ids: &[Uuid]) {
    assert!(slices_equal_ignore_order(&get_dirty_ids(db, true), ids));
}

pub fn assert_repo_integrity(db: &Config) {
    integrity_service::test_repo_integrity(db).unwrap();
}

pub fn assert_all_paths(db: &Config, root: &DecryptedFileMetadata, expected_paths: &[&str]) {
    if expected_paths.iter().any(|&path| !path.starts_with('/')) {
        panic!(
            "improper call to test_utils::assert_all_paths; all paths in expected_paths must begin with '/'. expected_paths={:?}",
            expected_paths
        );
    }
    let mut expected_paths: Vec<String> = expected_paths
        .iter()
        .map(|&path| String::from(path))
        .collect();
    let mut actual_paths: Vec<String> = crate::list_paths(db, None)
        .unwrap()
        .iter()
        .map(|path| String::from(&path[root.decrypted_name.len()..]))
        .collect();
    actual_paths.sort();
    expected_paths.sort();
    if actual_paths != expected_paths {
        panic!(
            "paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn assert_all_document_contents(
    db: &Config,
    root: &DecryptedFileMetadata,
    expected_contents_by_path: &[(&str, &[u8])],
) {
    let expected_contents_by_path = expected_contents_by_path
        .iter()
        .map(|&(path, contents)| (root.decrypted_name.clone() + path, contents.to_vec()))
        .collect::<Vec<(String, Vec<u8>)>>();
    let actual_contents_by_path = crate::list_paths(db, Some(path_service::Filter::DocumentsOnly))
        .unwrap()
        .iter()
        .map(|path| {
            (
                path.clone(),
                crate::read_document(db, crate::get_file_by_path(db, path).unwrap().id).unwrap(),
            )
        })
        .collect::<Vec<(String, Vec<u8>)>>();
    if !slices_equal_ignore_order(&actual_contents_by_path, &expected_contents_by_path) {
        panic!(
            "document contents did not match expectation. expected={:?}; actual={:?}",
            expected_contents_by_path
                .into_iter()
                .map(|(path, contents)| (path, String::from_utf8_lossy(&contents).to_string()))
                .collect::<Vec<(String, String)>>(),
            actual_contents_by_path
                .into_iter()
                .map(|(path, contents)| (path, String::from_utf8_lossy(&contents).to_string()))
                .collect::<Vec<(String, String)>>(),
        );
    }
}

pub fn assert_deleted_files_pruned(db: &Config) {
    for source in [RepoSource::Local, RepoSource::Base] {
        let all_metadata = file_service::get_all_metadata(db, source).unwrap();
        let not_deleted_metadata = file_service::get_all_not_deleted_metadata(db, source).unwrap();
        if !slices_equal_ignore_order(&all_metadata, &not_deleted_metadata) {
            panic!(
                "some deleted files are not pruned. not_deleted_metadata={:?}; all_metadata={:?}",
                not_deleted_metadata, all_metadata
            );
        }
    }
}

pub fn make_new_client(db: &Config) -> Config {
    let new_client = test_config();
    crate::import_account(&new_client, &crate::export_account(db).unwrap()).unwrap();
    new_client
}

pub fn make_and_sync_new_client(db: &Config) -> Config {
    let new_client = test_config();
    crate::import_account(&new_client, &crate::export_account(db).unwrap()).unwrap();
    crate::sync_all(&new_client, None).unwrap();
    new_client
}

pub fn assert_new_synced_client_dbs_eq(db: &Config) {
    let new_client = make_and_sync_new_client(db);
    assert_repo_integrity(&new_client);
    assert_dbs_eq(db, &new_client);
}

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
macro_rules! assert_get_updates_required {
    ($actual:expr) => {
        assert_matches!(
            $actual,
            Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
                FileMetadataUpsertsError::GetUpdatesRequired
            ))
        );
    };
}

#[macro_export]
macro_rules! path {
    ($account:expr, $path:expr) => {{
        &format!("{}/{}", $account.username, $path)
    }};
}

pub fn path(root: &DecryptedFileMetadata, path: &str) -> String {
    root.decrypted_name.clone() + path
}

pub fn create_account(db: &Config) -> (Account, DecryptedFileMetadata) {
    let generated_account = generate_account();
    (
        crate::create_account(db, &generated_account.username, &generated_account.api_url).unwrap(),
        crate::get_root(db).unwrap(),
    )
}

pub fn test_config() -> Config {
    Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4()),
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

pub fn generate_root_metadata(account: &Account) -> (EncryptedFileMetadata, AESKey) {
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
        EncryptedFileMetadata {
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
    parent: &EncryptedFileMetadata,
    parent_key: &AESKey,
    file_type: FileType,
) -> (EncryptedFileMetadata, AESKey) {
    let id = Uuid::new_v4();
    let file_key = symkey::generate_key();
    (
        EncryptedFileMetadata {
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
    symkey::decrypt(key, to_decrypt).unwrap()
}

pub fn assert_dbs_eq(db1: &Config, db2: &Config) {
    assert_eq!(
        account_repo::get(db1).unwrap(),
        account_repo::get(db2).unwrap()
    );

    assert_eq!(
        db_version_repo::maybe_get(db1).unwrap(),
        db_version_repo::maybe_get(db2).unwrap()
    );

    assert_eq!(
        root_repo::maybe_get(db1).unwrap(),
        root_repo::maybe_get(db2).unwrap()
    );

    assert_eq!(
        file_service::get_all_metadata_state(db1).unwrap(),
        file_service::get_all_metadata_state(db2).unwrap()
    );

    assert_eq!(
        file_service::get_all_document_state(db1).unwrap(),
        file_service::get_all_document_state(db2).unwrap()
    );
}

pub fn dbs_equal(db1: &Config, db2: &Config) -> bool {
    account_repo::get(db1).unwrap() == account_repo::get(db2).unwrap()
        && db_version_repo::maybe_get(db1).unwrap() == db_version_repo::maybe_get(db2).unwrap()
        && root_repo::maybe_get(db1).unwrap() == root_repo::maybe_get(db2).unwrap()
        && file_service::get_all_metadata_state(db1).unwrap()
            == file_service::get_all_metadata_state(db2).unwrap()
        && file_service::get_all_document_state(db1).unwrap()
            == file_service::get_all_document_state(db2).unwrap()
}

fn get_frequencies<T: Hash + Eq>(a: &[T]) -> HashMap<&T, i32> {
    let mut result = HashMap::new();
    for element in a {
        if let Some(count) = result.get_mut(element) {
            *count += 1;
        } else {
            result.insert(element, 1);
        }
    }
    result
}

pub fn slices_equal_ignore_order<T: Hash + Eq>(a: &[T], b: &[T]) -> bool {
    get_frequencies(a) == get_frequencies(b)
}

#[cfg(test)]
mod unit_tests {
    use super::super::test_utils;

    use std::array::IntoIter;
    use std::collections::HashMap;
    use std::iter::FromIterator;

    #[test]
    fn test_get_frequencies() {
        let expected = HashMap::<&i32, i32>::from_iter(IntoIter::new([(&0, 1), (&1, 3), (&2, 2)]));
        let result = test_utils::get_frequencies(&[0, 1, 1, 1, 2, 2]);
        assert_eq!(expected, result);
    }

    #[test]
    fn slices_equal_ignore_order_empty() {
        assert!(test_utils::slices_equal_ignore_order::<i32>(&[], &[]));
    }

    #[test]
    fn slices_equal_ignore_order_single() {
        assert!(test_utils::slices_equal_ignore_order::<i32>(&[69], &[69]));
    }

    #[test]
    fn slices_equal_ignore_order_single_nonequal() {
        assert!(!test_utils::slices_equal_ignore_order::<i32>(&[69], &[420]));
    }

    #[test]
    fn slices_equal_ignore_order_distinct() {
        assert!(test_utils::slices_equal_ignore_order::<i32>(
            &[69, 420, 69420],
            &[69420, 69, 420]
        ));
    }

    #[test]
    fn slices_equal_ignore_order_distinct_nonequal() {
        assert!(!test_utils::slices_equal_ignore_order::<i32>(
            &[69, 420, 69420],
            &[42069, 69, 420]
        ));
    }

    #[test]
    fn slices_equal_ignore_order_distinct_subset() {
        assert!(!test_utils::slices_equal_ignore_order::<i32>(
            &[69, 420, 69420],
            &[69, 420]
        ));
    }

    #[test]
    fn slices_equal_ignore_order_repeats() {
        assert!(test_utils::slices_equal_ignore_order::<i32>(
            &[69, 420, 420],
            &[420, 69, 420]
        ));
    }

    #[test]
    fn slices_equal_ignore_order_different_repeats() {
        assert!(!test_utils::slices_equal_ignore_order::<i32>(
            &[69, 420, 420],
            &[420, 69, 69]
        ));
    }

    #[test]
    fn slices_equal_ignore_order_repeats_subset() {
        assert!(!test_utils::slices_equal_ignore_order::<i32>(
            &[69, 420, 420],
            &[420, 69]
        ));
    }
}
