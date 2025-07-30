pub mod assert;

use itertools::Itertools as _;
use lb_rs::Lb;
use lb_rs::model::api::{PaymentMethod, StripeAccountTier};
use lb_rs::model::core_config::Config;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::work_unit::WorkUnit;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::{env, fs};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn test_config() -> Config {
    Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4()),
        logs: false,
        stdout_logs: false,
        colored_logs: false,
        background_work: false,
    }
}

pub async fn test_core() -> Lb {
    Lb::init(test_config()).await.unwrap()
}

pub async fn test_core_from(core: &Lb) -> Lb {
    let account_string = core.export_account_private_key().unwrap();
    let core = test_core().await;
    core.import_account(&account_string, Some(&url()))
        .await
        .unwrap();
    core.sync(None).await.unwrap();
    core
}

pub async fn test_core_with_account() -> Lb {
    let core = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
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

pub async fn write_path(c: &Lb, path: &str, content: &[u8]) -> Result<(), String> {
    let target = c.get_by_path(path).await.map_err(err_to_string)?;
    c.write_document(target.id, content)
        .await
        .map_err(err_to_string)
}

pub async fn delete_path(c: &Lb, path: &str) -> Result<(), String> {
    let target = c.get_by_path(path).await.map_err(err_to_string)?;
    c.delete(&target.id).await.map_err(err_to_string)
}

pub async fn move_by_path(c: &Lb, src: &str, dest: &str) -> Result<(), String> {
    let src = c.get_by_path(src).await.map_err(err_to_string)?;
    let dest = c.get_by_path(dest).await.map_err(err_to_string)?;
    c.move_file(&src.id, &dest.id).await.map_err(err_to_string)
}

pub async fn rename_path(c: &Lb, path: &str, new_name: &str) -> Result<(), String> {
    let target = c.get_by_path(path).await.map_err(err_to_string)?;
    c.rename_file(&target.id, new_name)
        .await
        .map_err(err_to_string)
}

pub async fn another_client(c: &Lb) -> Lb {
    let account_string = c.export_account_private_key().unwrap();
    let new_core = test_core().await;
    new_core
        .import_account(&account_string, Some(&url()))
        .await
        .unwrap();
    new_core
}

fn err_to_string<E: Debug>(e: E) -> String {
    format!("{}: {:?}", std::any::type_name::<E>(), e)
}

pub async fn get_dirty_ids(lb: &Lb, server: bool) -> Vec<Uuid> {
    lb.calculate_work()
        .await
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

pub async fn dbs_equal(left: &Lb, right: &Lb) -> bool {
    let mut left_tx = left.begin_tx().await;
    let mut right_tx = right.begin_tx().await;

    right_tx.db().account.get() == left_tx.db().account.get()
        && right_tx.db().root.get() == left_tx.db().root.get()
        && right_tx.db().local_metadata.get() == left_tx.db().local_metadata.get()
        && right_tx.db().base_metadata.get() == left_tx.db().base_metadata.get()
}

pub async fn assert_dbs_equal(left: &Lb, right: &Lb) {
    let mut left_tx = left.begin_tx().await;
    let mut right_tx = right.begin_tx().await;

    assert_eq!(left_tx.db().account.get(), right_tx.db().account.get());
    assert_eq!(left_tx.db().root.get(), right_tx.db().root.get());
    assert_eq!(left_tx.db().base_metadata.get(), right_tx.db().base_metadata.get());
    assert_eq!(left_tx.db().local_metadata.get(), right_tx.db().local_metadata.get());
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
    let path = lb_rs::io::docs::namespace_path(&PathBuf::from(db.writeable_path.clone()));
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
