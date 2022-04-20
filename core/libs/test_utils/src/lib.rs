use chrono::Datelike;
use hmdb::transaction::Transaction;
use itertools::Itertools;
use lockbook_core::model::repo::RepoSource;
use lockbook_core::repo::schema::Tx;
use lockbook_core::service::api_service::ApiError;
use lockbook_core::service::path_service::Filter::DocumentsOnly;
use lockbook_core::{Config, LbCore};
use lockbook_models::api::{AccountTier, FileMetadataUpsertsError, PaymentMethod};
use lockbook_models::file_metadata::{DecryptedFileMetadata, EncryptedFileMetadata};
use lockbook_models::tree::{FileMetaExt, FileMetadata};
use lockbook_models::work_unit::WorkUnit;
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::hash::Hash;
use uuid::Uuid;

pub fn test_config() -> Config {
    Config { writeable_path: format!("/tmp/{}", Uuid::new_v4()), logs: false }
}

pub fn test_core() -> LbCore {
    LbCore::init(&test_config()).unwrap()
}

pub fn test_core_from(core: &LbCore) -> LbCore {
    let account_string = core.export_account().unwrap();
    let core = test_core();
    core.import_account(&account_string).unwrap();
    core.sync(None).unwrap();
    core
}

pub fn test_core_with_account() -> LbCore {
    let core = test_core();
    core.create_account(&random_name(), &url()).unwrap();
    core
}

pub fn url() -> String {
    env::var("API_URL").unwrap_or_else(|_| "http://localhost:8000".to_string())
}

pub fn random_name() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn path(core: &LbCore, path: &str) -> String {
    let root = core.get_root().unwrap().name();
    format!("{}/{}", root, path)
}

pub const UPDATES_REQ: Result<(), ApiError<FileMetadataUpsertsError>> =
    Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
        FileMetadataUpsertsError::GetUpdatesRequired,
    ));

pub enum Operation<'a> {
    Client { client_num: usize },
    Sync { client_num: usize },
    Create { client_num: usize, path: &'a str },
    Rename { client_num: usize, path: &'a str, new_name: &'a str },
    Move { client_num: usize, path: &'a str, new_parent_path: &'a str },
    Delete { client_num: usize, path: &'a str },
    Edit { client_num: usize, path: &'a str, content: &'a [u8] },
    Custom { f: &'a dyn Fn(&[LbCore], &DecryptedFileMetadata) },
}

pub fn run(ops: &[Operation]) {
    let mut cores = vec![test_core()];
    cores[0].create_account(&random_name(), &url());
    let root = cores[0].get_root().unwrap();

    let ensure_client_exists = |clients: &mut Vec<LbCore>, client_num: &usize| {
        if *client_num > clients.len() - 1 {
            let account_string = clients[0].export_account().unwrap();
            let core = test_core();
            core.import_account(&account_string).unwrap();
            clients.push(core)
        }
    };

    for op in ops {
        match op {
            Operation::Client { client_num } => {
                ensure_client_exists(&mut cores, client_num);
            }
            Operation::Sync { client_num } => {
                || -> Result<_, String> {
                    ensure_client_exists(&mut cores, client_num);
                    cores[*client_num].sync(None).map_err(err_to_string)
                }()
                .unwrap_or_else(|_| panic!("Operation::Sync error. client_num={:?}", client_num));
            }
            Operation::Create { client_num, path } => {
                || -> Result<_, String> {
                    let core = &cores[*client_num];
                    let path = root.decrypted_name.clone() + "/" + path;
                    core.create_at_path(&path).map_err(err_to_string)
                }()
                .unwrap_or_else(|_| {
                    panic!("Operation::Create error. client_num={:?}, path={:?}", client_num, path)
                });
            }
            Operation::Rename { client_num, path, new_name } => {
                || -> Result<_, String> {
                    let core = &cores[*client_num];
                    let path = root.decrypted_name.clone() + "/" + path;
                    let target = core.get_by_path(&path).map_err(err_to_string)?;
                    core.rename_file(target.id, new_name).map_err(err_to_string)
                }()
                .unwrap_or_else(|_| {
                    panic!(
                        "Operation::Rename error. client_num={:?}, path={:?}, new_name={:?}",
                        client_num, path, new_name
                    )
                });
            }
            Operation::Move { client_num, path, new_parent_path } => {
                || -> Result<_, String> {
                    let core = &cores[*client_num];
                    let path = core.get_root().unwrap().decrypted_name.clone() + "/" + path;
                    let new_parent_path = root.decrypted_name.clone() + "/" + new_parent_path;
                    let target = core.get_by_path(&path).map_err(err_to_string)?;
                    let new_parent = core.get_by_path(&new_parent_path).map_err(err_to_string)?;
                    core.move_file(target.id, new_parent.id)
                        .map_err(err_to_string)
                }()
                .unwrap_or_else(|_| {
                    panic!(
                        "Operation::Move error. client_num={:?}, path={:?}, new_parent_path={:?}",
                        client_num, path, new_parent_path
                    )
                });
            }
            Operation::Delete { client_num, path } => {
                || -> Result<_, String> {
                    let core = &cores[*client_num];
                    let path = root.decrypted_name.clone() + "/" + path;
                    let target = core.get_by_path(&path).map_err(err_to_string)?;
                    core.delete_file(target.id).map_err(err_to_string)
                }()
                .unwrap_or_else(|_| {
                    panic!("Operation::Delete error. client_num={:?}, path={:?}", client_num, path)
                });
            }
            Operation::Edit { client_num, path, content } => {
                || -> Result<_, String> {
                    let core = &cores[*client_num];
                    let path = root.decrypted_name.clone() + "/" + path;
                    let target = core.get_by_path(&path).map_err(err_to_string)?;
                    core.write_document(target.id, content)
                        .map_err(err_to_string)
                }()
                .unwrap_or_else(|_| {
                    panic!(
                        "Operation::Edit error. client_num={:?}, path={:?}, content={:?}",
                        client_num, path, content
                    )
                });
            }
            Operation::Custom { f } => {
                f(&cores, &root);
            }
        }
    }
}

pub fn assert_all_paths(core: &LbCore, root: &DecryptedFileMetadata, expected_paths: &[&str]) {
    if expected_paths.iter().any(|&path| path.starts_with('/')) {
        panic!(
            "improper call to test_utils::assert_all_paths; all paths must not begin with '/'. expected_paths={:?}",
            expected_paths
        );
    }
    let mut expected_paths: Vec<String> = expected_paths
        .iter()
        .map(|&path| String::from(path))
        .collect();
    let mut actual_paths: Vec<String> = core
        .list_paths(None)
        .unwrap()
        .iter()
        .map(|path| String::from(&path[root.decrypted_name.len() + 1..]))
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

fn err_to_string<E: Debug>(e: E) -> String {
    format!("{}: {:?}", std::any::type_name::<E>(), e)
}

pub fn get_dirty_ids(db: &LbCore, server: bool) -> Vec<Uuid> {
    db.calculate_work()
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

pub fn assert_local_work_ids(db: &LbCore, ids: &[Uuid]) {
    assert!(slices_equal_ignore_order(&get_dirty_ids(db, false), ids));
}

pub fn assert_local_work_paths(
    db: &LbCore, root: &DecryptedFileMetadata, expected_paths: &[&'static str],
) {
    let all_local_files = db
        .db
        .transaction(|tx| tx.get_all_metadata(RepoSource::Local))
        .unwrap()
        .unwrap();

    let mut expected_paths = expected_paths.to_vec();
    let mut actual_paths: Vec<String> = get_dirty_ids(db, false)
        .iter()
        .map(|&id| {
            db.db
                .transaction(|tx| Tx::path_by_id_helper(&all_local_files, id))
                .unwrap()
                .unwrap()
        })
        .map(|path| String::from(&path[root.decrypted_name.len() + 1..]))
        .collect();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn assert_server_work_paths(
    db: &LbCore, root: &DecryptedFileMetadata, expected_paths: &[&'static str],
) {
    let staged = db
        .db
        .transaction(|tx| {
            let all_local_files = tx.get_all_metadata(RepoSource::Local).unwrap();
            let new_server_files = tx
                .calculate_work(&db.config)
                .unwrap()
                .work_units
                .into_iter()
                .filter_map(|wu| match wu {
                    WorkUnit::ServerChange { metadata } => Some(metadata),
                    _ => None,
                })
                .filter(|f| all_local_files.maybe_find(f.id).is_none())
                .collect::<Vec<DecryptedFileMetadata>>();
            all_local_files
                .stage(&new_server_files)
                .into_iter()
                .map(|s| s.0)
                .collect::<Vec<DecryptedFileMetadata>>()
        })
        .unwrap();

    let mut expected_paths = expected_paths.to_vec();
    let mut actual_paths: Vec<String> = get_dirty_ids(db, true)
        .iter()
        .map(|&id| Tx::path_by_id_helper(&staged, id).unwrap())
        .map(|path| String::from(&path[root.decrypted_name.len() + 1..]))
        .collect();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn assert_server_work_ids(db: &LbCore, ids: &[Uuid]) {
    assert!(slices_equal_ignore_order(&get_dirty_ids(db, true), ids));
}

pub fn assert_all_document_contents(
    db: &LbCore, root: &DecryptedFileMetadata, expected_contents_by_path: &[(&str, &[u8])],
) {
    let expected_contents_by_path = expected_contents_by_path
        .iter()
        .map(|&(path, contents)| (root.decrypted_name.clone() + "/" + path, contents.to_vec()))
        .collect::<Vec<(String, Vec<u8>)>>();
    let actual_contents_by_path = db
        .list_paths(Some(DocumentsOnly))
        .unwrap()
        .iter()
        .map(|path| (path.clone(), db.read_document(db.get_by_path(path).unwrap().id).unwrap()))
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
pub fn assert_deleted_files_pruned(core: &LbCore) {
    core.db.transaction(|tx| {
        for source in [RepoSource::Local, RepoSource::Base] {
            let all_metadata = tx.get_all_metadata(source).unwrap();
            let not_deleted_metadata = tx.get_all_not_deleted_metadata(source).unwrap();
            if !slices_equal_ignore_order(&all_metadata, &not_deleted_metadata) {
                panic!(
                    "some deleted files are not pruned. not_deleted_metadata={:?}; all_metadata={:?}",
                    not_deleted_metadata, all_metadata
                );
            }
        }
    }).unwrap();
}

/// Compare dbs for key equality don't compare last synced.
pub fn assert_dbs_eq(left: &LbCore, right: &LbCore) {
    keys_match(&left.db.account.get_all().unwrap(), &right.db.account.get_all().unwrap());
    keys_match(&left.db.root.get_all().unwrap(), &right.db.root.get_all().unwrap());
    keys_match(&left.db.local_digest.get_all().unwrap(), &right.db.local_digest.get_all().unwrap());
    keys_match(&left.db.base_digest.get_all().unwrap(), &right.db.base_digest.get_all().unwrap());
    keys_match(
        &left.db.local_metadata.get_all().unwrap(),
        &right.db.local_metadata.get_all().unwrap(),
    );
    keys_match(
        &left.db.base_metadata.get_all().unwrap(),
        &right.db.base_metadata.get_all().unwrap(),
    );
}

/// https://stackoverflow.com/questions/58615910/checking-two-hashmaps-for-identical-keyset-in-rust
fn keys_match<T: Eq + Hash, U, V>(map1: &HashMap<T, U>, map2: &HashMap<T, V>) -> bool {
    map1.len() == map2.len() && map1.keys().all(|k| map2.contains_key(k))
}

pub fn dbs_equal(left: &LbCore, right: &LbCore) -> bool {
    left.db.account.get_all().unwrap() == right.db.account.get_all().unwrap()
        && left.db.root.get_all().unwrap() == right.db.root.get_all().unwrap()
        && left.db.local_digest.get_all().unwrap() == right.db.local_digest.get_all().unwrap()
        && left.db.base_digest.get_all().unwrap() == right.db.base_digest.get_all().unwrap()
        && left.db.local_metadata.get_all().unwrap() == right.db.local_metadata.get_all().unwrap()
        && left.db.base_metadata.get_all().unwrap() == right.db.base_metadata.get_all().unwrap()
}

pub fn assert_new_synced_client_dbs_eq(core: &LbCore) {
    let new_client = test_core_from(core);
    assert_repo_integrity(&new_client);
    assert_dbs_eq(core, &new_client);
}

pub fn assert_repo_integrity(core: &LbCore) {
    core.validate().unwrap();
}

pub mod test_credit_cards {
    pub const GOOD: &str = "4242424242424242";
    pub const GOOD_LAST_4: &str = "4242";

    pub const INVALID_NUMBER: &str = "11111";

    pub mod decline {
        pub const GENERIC: &str = "4000000000000002";
        pub const INSUFFICIENT_FUNDS: &str = "4000000000009995";
        pub const LOST_CARD: &str = "4000000000009987";
        pub const EXPIRED_CARD: &str = "4000000000000069";
        pub const INCORRECT_CVC: &str = "4000000000000127";
        pub const PROCESSING_ERROR: &str = "4000000000000119";
        pub const INCORRECT_NUMBER: &str = "4242424242424241";
    }
}

pub mod test_card_info {
    pub const GENERIC_CVC: &str = "314";
    pub const GENERIC_EXP_MONTH: i32 = 8;
}

fn get_next_year() -> i32 {
    chrono::Utc::now().year() + 1
}

pub fn generate_premium_account_tier(
    card_number: &str, maybe_exp_year: Option<i32>, maybe_exp_month: Option<i32>,
    maybe_cvc: Option<&str>,
) -> AccountTier {
    AccountTier::Premium(PaymentMethod::NewCard {
        number: card_number.to_string(),
        exp_year: maybe_exp_year.unwrap_or_else(get_next_year),
        exp_month: maybe_exp_month.unwrap_or(test_card_info::GENERIC_EXP_MONTH),
        cvc: maybe_cvc.unwrap_or(test_card_info::GENERIC_CVC).to_string(),
    })
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
//
// #[cfg(test)]
// mod unit_tests {
//     use super::super::test_utils;
//
//     use std::collections::HashMap;
//     use std::iter::FromIterator;
//
//     #[test]
//     fn test_get_frequencies() {
//         let expected =
//             HashMap::<&i32, i32>::from_iter(IntoIterator::into_iter([(&0, 1), (&1, 3), (&2, 2)]));
//         let result = test_utils::get_frequencies(&[0, 1, 1, 1, 2, 2]);
//         assert_eq!(expected, result);
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_empty() {
//         assert!(test_utils::slices_equal_ignore_order::<i32>(&[], &[]));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_single() {
//         assert!(test_utils::slices_equal_ignore_order::<i32>(&[69], &[69]));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_single_nonequal() {
//         assert!(!test_utils::slices_equal_ignore_order::<i32>(&[69], &[420]));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_distinct() {
//         assert!(test_utils::slices_equal_ignore_order::<i32>(&[69, 420, 69420], &[69420, 69, 420]));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_distinct_nonequal() {
//         assert!(!test_utils::slices_equal_ignore_order::<i32>(
//             &[69, 420, 69420],
//             &[42069, 69, 420]
//         ));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_distinct_subset() {
//         assert!(!test_utils::slices_equal_ignore_order::<i32>(&[69, 420, 69420], &[69, 420]));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_repeats() {
//         assert!(test_utils::slices_equal_ignore_order::<i32>(&[69, 420, 420], &[420, 69, 420]));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_different_repeats() {
//         assert!(!test_utils::slices_equal_ignore_order::<i32>(&[69, 420, 420], &[420, 69, 69]));
//     }
//
//     #[test]
//     fn slices_equal_ignore_order_repeats_subset() {
//         assert!(!test_utils::slices_equal_ignore_order::<i32>(&[69, 420, 420], &[420, 69]));
//     }
// }
