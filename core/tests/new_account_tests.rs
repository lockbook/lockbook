use lockbook_core::pure_functions::files;
use lockbook_core::service::api_service::ApiError;
use lockbook_core::service::{api_service, file_encryption_service};
use lockbook_crypto::pubkey;
use lockbook_models::account::Account;
use lockbook_models::api::*;
use test_utils::*;

fn random_account() -> Account {
    Account { username: random_name(), api_url: url(), private_key: pubkey::generate_key() }
}

fn test_account(account: &Account) -> Result<NewAccountResponse, ApiError<NewAccountError>> {
    let root = files::create_root(account).unwrap();
    let root =
        file_encryption_service::encrypt_metadatum(&root.decrypted_access_key, &root).unwrap();
    api_service::request(account, NewAccountRequest::new(account, &root))
}
#[test]
fn new_account() {
    test_account(&random_account()).unwrap();
}

#[test]
fn new_account_duplicate_pk() {
    let first = random_account();
    test_account(&first).unwrap();

    let mut second = random_account();
    second.private_key = first.private_key;

    let result = test_account(&second);
    assert_matches!(
        result,
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::PublicKeyTaken))
    );
}

#[test]
fn new_account_duplicate_username() {
    let account = random_account();
    test_account(&account).unwrap();

    let mut account2 = random_account();
    account2.username = account.username;
    assert_matches!(
        test_account(&account2),
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::UsernameTaken))
    );
}

#[test]
fn new_account_invalid_username() {
    let mut account = random_account();
    account.username += " ";

    assert_matches!(
        test_account(&account),
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::InvalidUsername))
    );
}

#[test]
fn create_account_username_case() {
    let core = test_core_with_account();
    let mut account = core.get_account().unwrap();

    account.username = account.username.to_uppercase();
    account.private_key = pubkey::generate_key();

    assert_matches!(
        test_account(&account),
        Err(ApiError::Endpoint(NewAccountError::UsernameTaken))
    );
}
