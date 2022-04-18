use chrono::Datelike;
use std::env;

use hmdb::transaction::Transaction;
use lockbook_core::service::api_service::ApiError;
use lockbook_core::{Config, LbCore};
use lockbook_models::api::{AccountTier, FileMetadataUpsertsError, PaymentMethod};
use lockbook_models::file_metadata::EncryptedFileMetadata;
use lockbook_models::tree::FileMetadata;
use uuid::Uuid;

pub fn test_config() -> Config {
    Config { writeable_path: format!("/tmp/{}", Uuid::new_v4()), logs: false }
}

pub fn test_core() -> LbCore {
    LbCore::init(&test_config()).unwrap()
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

// pub fn enc_root(core: &LbCore) -> EncryptedFileMetadata {
//     core.db
//         .transaction(|tx| {
//             let id = tx.root().unwrap().id;
//             tx.base_metadata.get(&id).unwrap()
//         })
//         .unwrap()
// }

// pub enum Operation<'a> {
//     Client { client_num: usize },
//     Sync { client_num: usize },
//     Create { client_num: usize, path: &'a str },
//     Rename { client_num: usize, path: &'a str, new_name: &'a str },
//     Move { client_num: usize, path: &'a str, new_parent_path: &'a str },
//     Delete { client_num: usize, path: &'a str },
//     Edit { client_num: usize, path: &'a str, content: &'a [u8] },
//     Custom { f: &'a dyn Fn(&[(usize, Config)], &DecryptedFileMetadata) },
// }
//
// pub fn run(ops: &[Operation]) {
//     let mut clients = vec![test_core()];
//     create_account(&clients[0].1);
//
//     let ensure_client_exists = |clients: &mut Vec<(usize, Config)>, client_num: &usize| {
//         if !clients.iter().any(|(c, _)| c == client_num) {
//             clients.push((*client_num, make_new_client(&clients[0].1)))
//         }
//     };
//
//     for op in ops {
//         match op {
//             Operation::Client { client_num } => {
//                 ensure_client_exists(&mut clients, client_num);
//             }
//             Operation::Sync { client_num } => {
//                 || -> Result<_, String> {
//                     ensure_client_exists(&mut clients, client_num);
//                     let client = &clients.iter().find(|(c, _)| c == client_num).unwrap().1;
//                     crate::sync_all(client, None).map_err(err_to_string)
//                 }()
//                 .unwrap_or_else(|_| panic!("Operation::Sync error. client_num={:?}", client_num));
//             }
//             Operation::Create { client_num, path } => {
//                 || -> Result<_, String> {
//                     let path = root.decrypted_name.clone() + path;
//                     let client = &clients.iter().find(|(c, _)| c == client_num).unwrap().1;
//                     crate::create_file_at_path(client, &path).map_err(err_to_string)
//                 }()
//                 .unwrap_or_else(|_| {
//                     panic!("Operation::Create error. client_num={:?}, path={:?}", client_num, path)
//                 });
//             }
//             Operation::Rename { client_num, path, new_name } => {
//                 || -> Result<_, String> {
//                     let path = root.decrypted_name.clone() + path;
//                     let client = &clients.iter().find(|(c, _)| c == client_num).unwrap().1;
//                     let target = crate::get_file_by_path(client, &path).map_err(err_to_string)?;
//                     crate::rename_file(client, target.id, new_name).map_err(err_to_string)
//                 }()
//                 .unwrap_or_else(|_| {
//                     panic!(
//                         "Operation::Rename error. client_num={:?}, path={:?}, new_name={:?}",
//                         client_num, path, new_name
//                     )
//                 });
//             }
//             Operation::Move { client_num, path, new_parent_path } => {
//                 || -> Result<_, String> {
//                     let path = root.decrypted_name.clone() + path;
//                     let new_parent_path = root.decrypted_name.clone() + new_parent_path;
//                     let client = &clients.iter().find(|(c, _)| c == client_num).unwrap().1;
//                     let target = crate::get_file_by_path(client, &path).map_err(err_to_string)?;
//                     let new_parent =
//                         crate::get_file_by_path(client, &new_parent_path).map_err(err_to_string)?;
//                     crate::move_file(client, target.id, new_parent.id).map_err(err_to_string)
//                 }()
//                 .unwrap_or_else(|_| {
//                     panic!(
//                         "Operation::Move error. client_num={:?}, path={:?}, new_parent_path={:?}",
//                         client_num, path, new_parent_path
//                     )
//                 });
//             }
//             Operation::Delete { client_num, path } => {
//                 || -> Result<_, String> {
//                     let path = root.decrypted_name.clone() + path;
//                     let client = &clients.iter().find(|(c, _)| c == client_num).unwrap().1;
//                     let target = crate::get_file_by_path(client, &path).map_err(err_to_string)?;
//                     crate::delete_file(client, target.id).map_err(err_to_string)
//                 }()
//                 .unwrap_or_else(|_| {
//                     panic!("Operation::Delete error. client_num={:?}, path={:?}", client_num, path)
//                 });
//             }
//             Operation::Edit { client_num, path, content } => {
//                 || -> Result<_, String> {
//                     let path = root.decrypted_name.clone() + path;
//                     let client = &clients.iter().find(|(c, _)| c == client_num).unwrap().1;
//                     let target = crate::get_file_by_path(client, &path).map_err(err_to_string)?;
//                     crate::write_document(client, target.id, content).map_err(err_to_string)
//                 }()
//                 .unwrap_or_else(|_| {
//                     panic!(
//                         "Operation::Edit error. client_num={:?}, path={:?}, content={:?}",
//                         client_num, path, content
//                     )
//                 });
//             }
//             Operation::Custom { f } => {
//                 f(&clients, &root);
//             }
//         }
//     }
// }
//
// fn err_to_string<E: Debug>(e: E) -> String {
//     format!("{}: {:?}", std::any::type_name::<E>(), e)
// }
//
// #[macro_export]
// macro_rules! assert_dirty_ids {
//     ($db:expr, $n:literal) => {
//         assert_eq!(
//             sync_service::calculate_work(&$db)
//                 .unwrap()
//                 .work_units
//                 .into_iter()
//                 .map(|wu| wu.get_metadata().id)
//                 .unique()
//                 .count(),
//             $n
//         );
//     };
// }
//
// pub fn get_dirty_ids(db: &Config, server: bool) -> Vec<Uuid> {
//     sync_service::calculate_work(db)
//         .unwrap()
//         .work_units
//         .into_iter()
//         .filter_map(|wu| match wu {
//             WorkUnit::LocalChange { metadata } if !server => Some(metadata.id),
//             WorkUnit::ServerChange { metadata } if server => Some(metadata.id),
//             _ => None,
//         })
//         .unique()
//         .collect()
// }
//
// pub fn assert_local_work_ids(db: &Config, ids: &[Uuid]) {
//     assert!(slices_equal_ignore_order(&get_dirty_ids(db, false), ids));
// }
//
// // pub fn assert_local_work_paths(
// //     db: &Config, root: &DecryptedFileMetadata, expected_paths: &[&'static str],
// // ) {
// //     let all_local_files = file_service::get_all_metadata(db, RepoSource::Local).unwrap();
// //
// //     let mut expected_paths = expected_paths.to_vec();
// //     let mut actual_paths: Vec<String> = get_dirty_ids(db, false)
// //         .iter()
// //         .map(|&id| path_service::get_path_by_id_using_files(&all_local_files, id).unwrap())
// //         .map(|path| String::from(&path[root.decrypted_name.len()..]))
// //         .collect();
// //     actual_paths.sort_unstable();
// //     expected_paths.sort_unstable();
// //     if actual_paths != expected_paths {
// //         panic!(
// //             "paths did not match expectation. expected={:?}; actual={:?}",
// //             expected_paths, actual_paths
// //         );
// //     }
// // }
//
// pub fn assert_server_work_ids(db: &Config, ids: &[Uuid]) {
//     assert!(slices_equal_ignore_order(&get_dirty_ids(db, true), ids));
// }
//
// pub fn assert_all_document_contents(
//     db: &Config, root: &DecryptedFileMetadata, expected_contents_by_path: &[(&str, &[u8])],
// ) {
//     let expected_contents_by_path = expected_contents_by_path
//         .iter()
//         .map(|&(path, contents)| (root.decrypted_name.clone() + path, contents.to_vec()))
//         .collect::<Vec<(String, Vec<u8>)>>();
//     let actual_contents_by_path = crate::list_paths(db, Some(path_service::Filter::DocumentsOnly))
//         .unwrap()
//         .iter()
//         .map(|path| {
//             (
//                 path.clone(),
//                 crate::read_document(db, crate::get_file_by_path(db, path).unwrap().id).unwrap(),
//             )
//         })
//         .collect::<Vec<(String, Vec<u8>)>>();
//     if !slices_equal_ignore_order(&actual_contents_by_path, &expected_contents_by_path) {
//         panic!(
//             "document contents did not match expectation. expected={:?}; actual={:?}",
//             expected_contents_by_path
//                 .into_iter()
//                 .map(|(path, contents)| (path, String::from_utf8_lossy(&contents).to_string()))
//                 .collect::<Vec<(String, String)>>(),
//             actual_contents_by_path
//                 .into_iter()
//                 .map(|(path, contents)| (path, String::from_utf8_lossy(&contents).to_string()))
//                 .collect::<Vec<(String, String)>>(),
//         );
//     }
// }
//
// impl Tx<'_> {
//     pub fn assert_deleted_files_pruned(&self) {
//         for source in [RepoSource::Local, RepoSource::Base] {
//             let all_metadata = self.get_all_metadata(source).unwrap();
//             let not_deleted_metadata = self.get_all_not_deleted_metadata(source).unwrap();
//             if !slices_equal_ignore_order(&all_metadata, &not_deleted_metadata) {
//                 panic!(
//                     "some deleted files are not pruned. not_deleted_metadata={:?}; all_metadata={:?}",
//                     not_deleted_metadata, all_metadata
//                 );
//             }
//         }
//     }
//
//     pub fn assert_dbs_eq(&self, other: &Self) {
//         assert_eq!(self.account.get_all(), other.account.get_all());
//         assert_eq!(self.last_synced.get_all(), other.last_synced.get_all());
//         assert_eq!(self.root.get_all(), other.root.get_all());
//         assert_eq!(self.local_digest.get_all(), other.local_digest.get_all());
//         assert_eq!(self.base_digest.get_all(), other.base_digest.get_all());
//         assert_eq!(self.local_metadata.get_all(), other.local_metadata.get_all());
//         assert_eq!(self.base_metadata.get_all(), other.base_metadata.get_all());
//     }
//
//     pub fn dbs_equal(&self, other: &Self) -> bool {
//         self.account.get_all() == other.account.get_all()
//             && self.last_synced.get_all() == other.last_synced.get_all()
//             && self.root.get_all() == other.root.get_all()
//             && self.local_digest.get_all() == other.local_digest.get_all()
//             && self.base_digest.get_all() == other.base_digest.get_all()
//             && self.local_metadata.get_all() == other.local_metadata.get_all()
//             && self.base_metadata.get_all() == other.base_metadata.get_all()
//     }
//
//     pub fn assert_new_synced_client_dbs_eq(&self, db: &Config) {
//         let new_client = self.make_and_sync_new_client();
//         new_client
//             .db
//             .transaction(|tx| {
//                 tx.assert_repo_integrity(&new_client.config);
//                 tx.assert_dbs_eq(&self);
//             })
//             .unwrap();
//     }
//
//     pub fn make_and_sync_new_client(&self) -> LbCore {
//         let new_client = test_config();
//         let account_string = self.export_account().unwrap();
//
//         let new_db = LbCore::init(&new_client).unwrap();
//         new_db.import_account(&account_string).unwrap();
//         new_db.sync(None).unwrap();
//         new_db
//     }
//
//     pub fn assert_repo_integrity(&self, config: &Config) {
//         self.test_repo_integrity(config).unwrap();
//     }
// }
//
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

// pub const MAX_FILES_PER_BENCH: u64 = 6;
//
// pub const CREATE_FILES_BENCH_1: u64 = 1;
// pub const CREATE_FILES_BENCH_2: u64 = 10;
// pub const CREATE_FILES_BENCH_3: u64 = 100;
// pub const CREATE_FILES_BENCH_4: u64 = 500;
// pub const CREATE_FILES_BENCH_5: u64 = 1000;
// pub const CREATE_FILES_BENCH_6: u64 = 2000;
//
// pub fn random_filename() -> SecretFileName {
//     let name: String = Uuid::new_v4()
//         .to_string()
//         .chars()
//         .filter(|c| c.is_alphanumeric())
//         .collect();
//
//     symkey::encrypt_and_hmac(&symkey::generate_key(), &name).unwrap()
// }
//
// fn get_frequencies<T: Hash + Eq>(a: &[T]) -> HashMap<&T, i32> {
//     let mut result = HashMap::new();
//     for element in a {
//         if let Some(count) = result.get_mut(element) {
//             *count += 1;
//         } else {
//             result.insert(element, 1);
//         }
//     }
//     result
// }
//
// pub fn slices_equal_ignore_order<T: Hash + Eq>(a: &[T], b: &[T]) -> bool {
//     get_frequencies(a) == get_frequencies(b)
// }
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
