use lb_rs::service::api_service::{ApiError, Network, Requester};
use lockbook_shared::account::{Account, MAX_USERNAME_LENGTH};
use lockbook_shared::api::*;
use lockbook_shared::file_metadata::FileMetadata;
use lockbook_shared::pubkey;
use test_utils::*;

fn random_account() -> Account {
    Account { username: random_name(), api_url: url(), private_key: pubkey::generate_key() }
}

fn test_account(account: &Account) -> Result<NewAccountResponse, ApiError<NewAccountError>> {
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();
    Network::default().request(account, NewAccountRequest::new(account, &root))
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
fn new_account_username_too_long() {
    let mut account = random_account();
    account.username = "l".repeat(MAX_USERNAME_LENGTH + 1);

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
