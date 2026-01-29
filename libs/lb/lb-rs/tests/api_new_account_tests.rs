use lb_rs::io::network::{ApiError, Network};
use lb_rs::model::account::{Account, MAX_USERNAME_LENGTH};
use lb_rs::model::api::*;
use lb_rs::model::meta::Meta;
use lb_rs::model::pubkey;
use test_utils::*;

async fn random_account() -> Account {
    Account { username: random_name(), api_url: url(), private_key: pubkey::generate_key() }
}

async fn test_account(account: &Account) -> Result<NewAccountResponse, ApiError<NewAccountError>> {
    let root = Meta::create_root(account)
        .unwrap()
        .sign_with(account)
        .unwrap();
    Network::default()
        .request(account, NewAccountRequestV2::new(account, &root))
        .await
}

#[tokio::test]
async fn new_account() {
    test_account(&random_account().await).await.unwrap();
}

#[tokio::test]
async fn new_account_duplicate_pk() {
    let first = random_account().await;
    test_account(&first).await.unwrap();

    let mut second = random_account().await;
    second.private_key = first.private_key;

    let result = test_account(&second).await;
    assert_matches!(
        result,
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::PublicKeyTaken))
    );
}

#[tokio::test]
async fn new_account_duplicate_username() {
    let account = random_account().await;
    test_account(&account).await.unwrap();

    let mut account2 = random_account().await;
    account2.username = account.username;
    assert_matches!(
        test_account(&account2).await,
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::UsernameTaken))
    );
}

#[tokio::test]
async fn new_account_invalid_username() {
    let mut account = random_account().await;
    account.username += " ";

    assert_matches!(
        test_account(&account).await,
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::InvalidUsername))
    );
}

#[tokio::test]
async fn new_account_username_too_long() {
    let mut account = random_account().await;
    account.username = "l".repeat(MAX_USERNAME_LENGTH + 1);

    assert_matches!(
        test_account(&account).await,
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::InvalidUsername))
    );
}

#[tokio::test]
async fn create_account_username_case() {
    let core = test_core_with_account().await;
    let mut account = core.get_account().unwrap().clone();

    account.username = account.username.to_uppercase();
    account.private_key = pubkey::generate_key();

    assert_matches!(
        test_account(&account).await,
        Err(ApiError::Endpoint(NewAccountError::UsernameTaken))
    );
}
