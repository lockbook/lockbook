pub mod assert;

use itertools::Itertools;
use lb_rs::service::api_service::Requester;
use lb_rs::DocumentService;
use lb_rs::{Core, CoreLib};
use lockbook_shared::api::{PaymentMethod, StripeAccountTier};
use lockbook_shared::core_config::Config;
use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::document_repo;
use lockbook_shared::work_unit::WorkUnit;
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
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
    core.create_account(&random_name(), &url(), false).unwrap();
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

fn err_to_string<E: Debug>(e: E) -> String {
    format!("{}: {:?}", std::any::type_name::<E>(), e)
}

pub fn get_dirty_ids(db: &Core, server: bool) -> Vec<Uuid> {
    db.calculate_work()
        .unwrap()
        .work_units
        .into_iter()
        .filter_map(|wu| match wu {
            WorkUnit::LocalChange(id) if !server => Some(id),
            WorkUnit::ServerChange(id) if server => Some(id),
            _ => None,
        })
        .unique()
        .collect()
}

pub fn dbs_equal<Client: Requester, Docs: DocumentService>(
    left: &CoreLib<Client, Docs>, right: &CoreLib<Client, Docs>,
) -> bool {
    left.in_tx(|l| {
        Ok(right
            .in_tx(|r| {
                Ok(r.db.account.get() == l.db.account.get()
                    && r.db.root.get() == l.db.root.get()
                    && r.db.local_metadata.get() == l.db.local_metadata.get()
                    && r.db.base_metadata.get() == l.db.base_metadata.get())
            })
            .unwrap())
    })
    .unwrap()
}

pub fn assert_dbs_equal<Client: Requester, Docs: DocumentService>(
    left: &CoreLib<Client, Docs>, right: &CoreLib<Client, Docs>,
) {
    left.in_tx(|l| {
        right
            .in_tx(|r| {
                assert_eq!(l.db.account.get(), r.db.account.get());
                assert_eq!(l.db.root.get(), r.db.root.get());
                assert_eq!(l.db.base_metadata.get(), r.db.base_metadata.get());
                assert_eq!(l.db.local_metadata.get(), r.db.local_metadata.get());
                Ok(())
            })
            .unwrap();
        Ok(())
    })
    .unwrap();
}

pub fn doc_repo_get_all(config: &Config) -> Vec<EncryptedDocument> {
    let mut docs = vec![];
    for file in list_files(config) {
        let content = fs::read(file).unwrap();
        docs.push(bincode::deserialize(&content).unwrap());
    }
    docs
}

fn list_files(db: &Config) -> Vec<String> {
    let path = document_repo::namespace_path(&db.writeable_path);
    let path = Path::new(&path);

    match fs::read_dir(path) {
        Ok(rd) => {
            let mut file_names = rd
                .map(|dir_entry| {
                    let entry = dir_entry.unwrap().file_name().into_string().unwrap();
                    let mut path = PathBuf::from(path);
                    path.push(entry);
                    path.to_str().unwrap().to_string()
                })
                .collect::<Vec<String>>();
            file_names.sort();

            file_names
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
    OffsetDateTime::now_utc().year() + 1
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
