use lb_rs::logic::account::{Account, MAX_USERNAME_LENGTH};
use lb_rs::logic::pubkey;
use lb_rs::model::errors::CoreError;
use test_utils::*;

#[tokio::test]
async fn create_account_success() {
    let core = test_core();
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
}

#[tokio::test]
async fn create_account_success_with_welcome() {
    let core = test_core();
    core.create_account(&random_name(), &url(), true)
        .await
        .unwrap();
    let welcome_doc = core.get_by_path("welcome.md").await.unwrap().id;
    assert!(String::from_utf8_lossy(&core.read_document(welcome_doc).await.unwrap())
        .to_lowercase()
        .contains("welcome"));
}

#[tokio::test]
async fn create_account_invalid_url() {
    let core = test_core();
    let result = core
        .create_account(&random_name(), "https://bad-url.net", false)
        .await;
    assert!(matches!(result.unwrap_err().kind, CoreError::ServerUnreachable))
}

#[tokio::test]
async fn create_account_invalid_url_with_welcome() {
    let core = test_core();
    let result = core
        .create_account(&random_name(), "https://bad-url.net", true)
        .await;
    assert!(matches!(result.unwrap_err().kind, CoreError::ServerUnreachable))
}

#[tokio::test]
async fn create_account_username_taken() {
    let core1 = test_core();
    let core2 = test_core();
    let name = random_name();

    core1.create_account(&name, &url(), false).await.unwrap();

    let err = core2
        .create_account(&name, &url(), false)
        .await
        .unwrap_err();

    assert!(
        matches!(err.kind, CoreError::UsernameTaken),
        "Username \"{}\" should have caused a UsernameTaken error but instead was {:?}",
        &name,
        err
    )
}

#[tokio::test]
async fn create_account_invalid_username() {
    let core = test_core();

    let invalid_unames =
        ["", "i/o", "@me", "###", "+1", "💩", &"x".repeat(MAX_USERNAME_LENGTH + 1)];

    for &uname in &invalid_unames {
        let err = core.create_account(uname, &url(), false).await.unwrap_err();

        assert!(
            matches!(err.kind, CoreError::UsernameInvalid),
            "Username \"{}\" should have been InvalidUsername but instead was {:?}",
            uname,
            err
        )
    }
}

#[tokio::test]
async fn create_account_account_exists() {
    let core = &test_core();

    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    assert!(
        matches!(
            core.create_account(&random_name(), &url(), false)
                .await
                .unwrap_err()
                .kind,
            CoreError::AccountExists
        ),
        "This action should have failed with AccountAlreadyExists!",
    );
}

#[tokio::test]
async fn create_account_account_exists_case() {
    let core = test_core();
    let name = random_name();

    core.create_account(&name, &url(), false).await.unwrap();

    let core = test_core();
    assert!(matches!(
        core.create_account(&(name.to_uppercase()), &url(), false)
            .await
            .unwrap_err()
            .kind,
        CoreError::UsernameTaken
    ));
}

#[tokio::test]
async fn import_account_account_exists() {
    let mut core = test_core();

    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    let account_string = core.export_account().await.unwrap();

    assert!(matches!(
        core.import_account(&account_string).await.unwrap_err().kind,
        CoreError::AccountExists
    ));
}

#[tokio::test]
async fn import_account_corrupted() {
    let mut core = test_core();

    assert!(matches!(
        core.import_account("clearly a bad account string")
            .await
            .unwrap_err()
            .kind,
        CoreError::AccountStringCorrupted
    ));
}

#[tokio::test]
async fn import_account_corrupted_base64() {
    let mut core = test_core();

    base64::decode("clearlyabadaccountstring").unwrap();
    assert!(matches!(
        core.import_account("clearlyabadaccountstring")
            .await
            .unwrap_err()
            .kind,
        CoreError::AccountStringCorrupted
    ));
}

#[tokio::test]
async fn import_account_nonexistent() {
    let core1 = test_core();

    core1
        .create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let core2 = test_core();
    let account =
        Account { api_url: url(), username: random_name(), private_key: pubkey::generate_key() };

    let mut tx = core2.begin_tx().await;
    tx.db().account.insert(account).unwrap();

    let account_string = core2.export_account().await.unwrap();

    let mut core3 = test_core();
    assert!(matches!(
        core3
            .import_account(&account_string)
            .await
            .unwrap_err()
            .kind,
        CoreError::AccountNonexistent
    ));
}

#[tokio::test]
async fn import_account_public_key_mismatch() {
    let bad_account_string = {
        let core1 = test_core();
        let core2 = test_core();
        let account1 = core1
            .create_account(&random_name(), &url(), false)
            .await
            .unwrap();
        let mut account2 = core2
            .create_account(&random_name(), &url(), false)
            .await
            .unwrap();
        account2.username = account1.username;

        let mut tx = core2.begin_tx().await;
        tx.db().account.insert(account2).unwrap();

        core2.export_account().await.unwrap()
    };

    let mut core3 = test_core();

    assert!(matches!(
        core3
            .import_account(&bad_account_string)
            .await
            .unwrap_err()
            .kind,
        CoreError::UsernamePublicKeyMismatch
    ));
}

#[tokio::test]
async fn export_account() {
    let core = test_core();
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    core.export_account().await.unwrap();
    core.export_account_qr().await.unwrap();
}

#[tokio::test]
async fn nonzero_root_version() {
    let core = test_core();
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    assert!(core.root().await.unwrap().last_modified > 0);
}
