use lb_rs::model::account::{Account, MAX_USERNAME_LENGTH};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::pubkey;
use test_utils::*;

#[tokio::test]
async fn create_account_success() {
    let core = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
}

#[tokio::test]
async fn create_account_success_with_welcome() {
    let core = test_core().await;
    core.create_account(&random_name(), &url(), true)
        .await
        .unwrap();
    let welcome_doc = core.get_by_path("welcome.md").await.unwrap().id;
    assert!(
        String::from_utf8_lossy(&core.read_document(welcome_doc, false).await.unwrap())
            .to_lowercase()
            .contains("welcome")
    );
}

#[tokio::test]
async fn create_account_invalid_url() {
    let core = test_core().await;
    let result = core
        .create_account(&random_name(), "https://bad-url.net", false)
        .await;
    assert!(matches!(result.unwrap_err().kind, LbErrKind::ServerUnreachable))
}

#[tokio::test]
async fn create_account_invalid_url_with_welcome() {
    let core = test_core().await;
    let result = core
        .create_account(&random_name(), "https://bad-url.net", true)
        .await;
    assert!(matches!(result.unwrap_err().kind, LbErrKind::ServerUnreachable))
}

#[tokio::test]
async fn create_account_username_taken() {
    let core1 = test_core().await;
    let core2 = test_core().await;
    let name = random_name();

    core1.create_account(&name, &url(), false).await.unwrap();

    let err = core2
        .create_account(&name, &url(), false)
        .await
        .unwrap_err();

    assert!(
        matches!(err.kind, LbErrKind::UsernameTaken),
        "Username \"{}\" should have caused a UsernameTaken error but instead was {:?}",
        &name,
        err
    )
}

#[tokio::test]
async fn create_account_invalid_username() {
    let core = test_core().await;

    let invalid_unames =
        ["", "i/o", "@me", "###", "+1", "💩", &"x".repeat(MAX_USERNAME_LENGTH + 1)];

    for &uname in &invalid_unames {
        let err = core.create_account(uname, &url(), false).await.unwrap_err();

        assert!(
            matches!(err.kind, LbErrKind::UsernameInvalid),
            "Username \"{uname}\" should have been InvalidUsername but instead was {err:?}"
        )
    }
}

#[tokio::test]
async fn create_account_account_exists() {
    let core = &test_core().await;

    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    assert!(
        matches!(
            core.create_account(&random_name(), &url(), false)
                .await
                .unwrap_err()
                .kind,
            LbErrKind::AccountExists
        ),
        "This action should have failed with AccountAlreadyExists!",
    );
}

#[tokio::test]
async fn create_account_account_exists_case() {
    let core = test_core().await;
    let name = random_name();

    core.create_account(&name, &url(), false).await.unwrap();

    let core = test_core().await;
    assert!(matches!(
        core.create_account(&(name.to_uppercase()), &url(), false)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::UsernameTaken
    ));
}

#[tokio::test]
async fn import_account_account_exists() {
    let core = test_core().await;

    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    let account_string = core.export_account_private_key().unwrap();

    assert!(matches!(
        core.import_account(&account_string, Some(&url()))
            .await
            .unwrap_err()
            .kind,
        LbErrKind::AccountExists
    ));
}

#[tokio::test]
async fn import_account_corrupted() {
    let core = test_core().await;

    assert!(matches!(
        core.import_account("clearly a bad account string", Some(&url()))
            .await
            .unwrap_err()
            .kind,
        LbErrKind::AccountStringCorrupted
    ));
}

#[tokio::test]
async fn import_account_corrupted_base64() {
    let core = test_core().await;

    base64::decode("clearlyabadaccountstring").unwrap();
    assert!(matches!(
        core.import_account("clearlyabadaccountstring", Some(&url()))
            .await
            .unwrap_err()
            .kind,
        LbErrKind::AccountStringCorrupted
    ));
}

#[tokio::test]
async fn import_account_nonexistent() {
    let core1 = test_core().await;

    core1
        .create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let core2 = test_core().await;
    let account =
        Account { api_url: url(), username: random_name(), private_key: pubkey::generate_key() };

    let mut tx = core2.begin_tx().await;
    tx.db().account.insert(account.clone()).unwrap();
    core2.keychain.cache_account(account).await.unwrap();

    let account_string = core2.export_account_private_key().unwrap();

    let core3 = test_core().await;
    assert!(matches!(
        core3
            .import_account(&account_string, Some(&url()))
            .await
            .unwrap_err()
            .kind,
        LbErrKind::AccountNonexistent
    ));
}

#[tokio::test]
async fn import_account_public_key_mismatch() {
    let bad_account_string = {
        let core1 = test_core().await;
        let core2 = test_core().await;
        let core3 = test_core().await;

        let account1 = core1
            .create_account(&random_name(), &url(), false)
            .await
            .unwrap();

        let mut account2 = core2
            .create_account(&random_name(), &url(), false)
            .await
            .unwrap();
        account2.username = account1.username;

        core3.keychain.cache_account(account2).await.unwrap();
        core3.export_account_private_key().unwrap()
    };

    let core4 = test_core().await;

    assert!(matches!(
        core4
            .import_account(&bad_account_string, Some(&url()))
            .await
            .unwrap_err()
            .kind,
        LbErrKind::UsernamePublicKeyMismatch
    ));
}

#[tokio::test]
async fn export_account() {
    let core = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    core.export_account_private_key().unwrap();
    core.export_account_qr().unwrap();
}

#[tokio::test]
async fn import_account_phrases() {
    let core1 = test_core().await;
    let account1 = core1
        .create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    let account_phrase = core1.export_account_phrase().unwrap();

    let core2 = test_core().await;
    let account2 = core2
        .import_account(&account_phrase, Some(&account1.api_url))
        .await
        .unwrap();

    assert_eq!(account1.private_key.serialize(), account2.private_key.serialize());
}

#[tokio::test]
async fn nonzero_root_version() {
    let core = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    assert!(core.root().await.unwrap().last_modified > 0);
}
