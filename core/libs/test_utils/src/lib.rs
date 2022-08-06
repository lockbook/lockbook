use chrono::Datelike;
use hmdb::transaction::Transaction;
use itertools::Itertools;
use lockbook_core::model::repo::RepoSource;
use lockbook_core::repo::schema::OneKey;
use lockbook_core::repo::{document_repo, local_storage};
use lockbook_core::{Config, Core};
use lockbook_shared::api::{PaymentMethod, StripeAccountTier};
use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::path_ops::Filter::DocumentsOnly;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::work_unit::WorkUnit;
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::hash::Hash;
use std::path::Path;
use uuid::Uuid;

pub fn test_config() -> Config {
    Config { writeable_path: format!("/tmp/{}", Uuid::new_v4()), logs: false, colored_logs: false }
}

pub fn test_core() -> Core {
    Core::init(&test_config()).unwrap()
}

pub fn test_core_from(core: &Core) -> Core {
    let account_string = core.export_account().unwrap();
    let core = test_core();
    core.import_account(&account_string).unwrap();
    core.sync(None).unwrap();
    core
}

pub fn test_core_with_account() -> Core {
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
//
// pub const UPDATES_REQ: Result<(), ApiError<FileMetadataUpsertsError>> =
//     Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
//         FileMetadataUpsertsError::GetUpdatesRequired,
//     ));

pub fn write_path(c: &Core, path: &str, content: &[u8]) -> Result<(), String> {
    let target = c.get_by_path(path).map_err(err_to_string)?;
    c.write_document(target.id, content).map_err(err_to_string)
}

pub fn delete_path(c: &Core, path: &str) -> Result<(), String> {
    let target = c.get_by_path(path).map_err(err_to_string)?;
    c.delete_file(target.id).map_err(err_to_string)
}

pub fn move_by_path(c: &Core, src: &str, dest: &str) -> Result<(), String> {
    let src = c.get_by_path(src).map_err(err_to_string)?;
    let dest = c.get_by_path(dest).map_err(err_to_string)?;
    c.move_file(src.id, dest.id).map_err(err_to_string)
}

pub fn rename_path(c: &Core, path: &str, new_name: &str) -> Result<(), String> {
    let target = c.get_by_path(path).map_err(err_to_string)?;
    c.rename_file(target.id, new_name).map_err(err_to_string)
}

pub fn another_client(c: &Core) -> Core {
    let account_string = c.export_account().unwrap();
    let new_core = test_core();
    new_core.import_account(&account_string).unwrap();
    new_core
}

pub fn assert_all_paths(core: &Core, expected_paths: &[&str]) {
    let mut expected_paths: Vec<String> = expected_paths
        .iter()
        .map(|&path| String::from(path))
        .collect();
    let mut actual_paths: Vec<String> = core.list_paths(None).unwrap();

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

pub fn get_dirty_ids(db: &Core, server: bool) -> Vec<Uuid> {
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

pub fn assert_local_work_ids(db: &Core, ids: &[Uuid]) {
    assert!(slices_equal_ignore_order(&get_dirty_ids(db, false), ids));
}

pub fn assert_local_work_paths(db: &Core, expected_paths: &[&'static str]) {
    let dirty = get_dirty_ids(db, false);

    let mut expected_paths = expected_paths.to_vec();
    let mut actual_paths = db
        .db
        .transaction(|tx| {
            let account = tx.account.get(&OneKey {}).unwrap();
            let mut local = tx.base_metadata.stage(&mut tx.local_metadata).to_lazy();
            dirty
                .iter()
                .map(|id| local.id_to_path(id, account))
                .collect::<Result<Vec<String>, _>>()
                .unwrap()
        })
        .unwrap();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn assert_server_work_paths(db: &Core, expected_paths: &[&'static str]) {
    let mut expected_paths = expected_paths.to_vec();
    let mut actual_paths = db
        .db
        .transaction(|tx| {
            let context = db.context(tx).unwrap();
            let account = context.tx.account.get(&OneKey {}).unwrap();
            let remote_changes = context.get_updates(account).unwrap().file_metadata;
            let mut remote = context.tx.base_metadata.stage(remote_changes).to_lazy();
            remote
                .tree
                .staged
                .owned_ids()
                .iter()
                .map(|id| remote.id_to_path(id, account))
                .collect::<Result<Vec<String>, _>>()
                .unwrap()
        })
        .unwrap();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn assert_server_work_ids(db: &Core, ids: &[Uuid]) {
    assert!(slices_equal_ignore_order(&get_dirty_ids(db, true), ids));
}

pub fn assert_all_document_contents(db: &Core, expected_contents_by_path: &[(&str, &[u8])]) {
    let expected_contents_by_path = expected_contents_by_path
        .iter()
        .map(|&(path, contents)| (path.to_string(), contents.to_vec()))
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

pub fn assert_deleted_files_pruned(core: &Core) {
    core.db
        .transaction(|tx| {
            let context = core.context(tx).unwrap();
            let mut base = context.tx.base_metadata.to_lazy();
            let deleted_base_ids = base
                .owned_ids()
                .into_iter()
                .filter(|id| base.calculate_deleted(id).unwrap())
                .collect::<Vec<Uuid>>();
            if !deleted_base_ids.is_empty() {
                panic!("some deleted files are not pruned:{:?}", deleted_base_ids);
            }
            let mut local = base.stage(&mut context.tx.local_metadata);
            let deleted_local_ids = local
                .owned_ids()
                .into_iter()
                .filter(|id| local.calculate_deleted(id).unwrap())
                .collect::<Vec<Uuid>>();
            if !deleted_local_ids.is_empty() {
                panic!("some deleted files are not pruned:{:?}", deleted_local_ids);
            }
        })
        .unwrap();
}

pub fn assert_dbs_eq(left: &Core, right: &Core) {
    assert_eq!(&left.db.account.get_all().unwrap(), &right.db.account.get_all().unwrap());
    assert_eq!(&left.db.root.get_all().unwrap(), &right.db.root.get_all().unwrap());
    assert_eq!(
        &left.db.local_metadata.get_all().unwrap(),
        &right.db.local_metadata.get_all().unwrap()
    );
    assert_eq!(
        &left.db.base_metadata.get_all().unwrap(),
        &right.db.base_metadata.get_all().unwrap()
    );
}

pub fn dbs_equal(left: &Core, right: &Core) -> bool {
    left.db.account.get_all().unwrap() == right.db.account.get_all().unwrap()
        && left.db.root.get_all().unwrap() == right.db.root.get_all().unwrap()
        && left.db.local_metadata.get_all().unwrap() == right.db.local_metadata.get_all().unwrap()
        && left.db.base_metadata.get_all().unwrap() == right.db.base_metadata.get_all().unwrap()
}

pub fn assert_new_synced_client_dbs_eq(core: &Core) {
    let new_client = test_core_from(core);
    assert_repo_integrity(&new_client);
    assert_dbs_eq(core, &new_client);
}

pub fn assert_repo_integrity(core: &Core) {
    core.validate().unwrap();
}

pub fn doc_repo_get_all(config: &Config, source: RepoSource) -> Vec<EncryptedDocument> {
    dump_local_storage::<_, Vec<u8>>(config, document_repo::namespace(source))
        .into_iter()
        .map(|s| bincode::deserialize(s.as_ref()).unwrap())
        .collect::<Vec<EncryptedDocument>>()
        .into_iter()
        .collect()
}

fn dump_local_storage<N, V>(db: &Config, namespace: N) -> Vec<V>
where
    N: AsRef<[u8]> + Copy,
    V: From<Vec<u8>>,
{
    let path_str = local_storage::namespace_path(db, namespace);
    let path = Path::new(&path_str);

    match fs::read_dir(path) {
        Ok(rd) => {
            let mut file_names = rd
                .map(|dir_entry| dir_entry.unwrap().file_name().into_string().unwrap())
                .collect::<Vec<String>>();
            file_names.sort();

            file_names
                .iter()
                .map(|file_name| {
                    local_storage::read(db, namespace, file_name)
                        .unwrap()
                        .unwrap()
                })
                .collect::<Vec<V>>()
        }
        Err(_) => Vec::new(),
    }
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
) -> StripeAccountTier {
    StripeAccountTier::Premium(PaymentMethod::NewCard {
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

#[cfg(test)]
mod unit_tests {
    use crate::*;
    use std::collections::HashMap;
    use std::iter::FromIterator;

    #[test]
    fn test_get_frequencies() {
        let expected =
            HashMap::<&i32, i32>::from_iter(IntoIterator::into_iter([(&0, 1), (&1, 3), (&2, 2)]));
        let result = get_frequencies(&[0, 1, 1, 1, 2, 2]);
        assert_eq!(expected, result);
    }

    #[test]
    fn slices_equal_ignore_order_empty() {
        assert!(slices_equal_ignore_order::<i32>(&[], &[]));
    }

    #[test]
    fn slices_equal_ignore_order_single() {
        assert!(slices_equal_ignore_order::<i32>(&[69], &[69]));
    }

    #[test]
    fn slices_equal_ignore_order_single_nonequal() {
        assert!(!slices_equal_ignore_order::<i32>(&[69], &[420]));
    }

    #[test]
    fn slices_equal_ignore_order_distinct() {
        assert!(slices_equal_ignore_order::<i32>(&[69, 420, 69420], &[69420, 69, 420]));
    }

    #[test]
    fn slices_equal_ignore_order_distinct_nonequal() {
        assert!(!slices_equal_ignore_order::<i32>(&[69, 420, 69420], &[42069, 69, 420]));
    }

    #[test]
    fn slices_equal_ignore_order_distinct_subset() {
        assert!(!slices_equal_ignore_order::<i32>(&[69, 420, 69420], &[69, 420]));
    }

    #[test]
    fn slices_equal_ignore_order_repeats() {
        assert!(slices_equal_ignore_order::<i32>(&[69, 420, 420], &[420, 69, 420]));
    }

    #[test]
    fn slices_equal_ignore_order_different_repeats() {
        assert!(!slices_equal_ignore_order::<i32>(&[69, 420, 420], &[420, 69, 69]));
    }

    #[test]
    fn slices_equal_ignore_order_repeats_subset() {
        assert!(!slices_equal_ignore_order::<i32>(&[69, 420, 420], &[420, 69]));
    }
}
