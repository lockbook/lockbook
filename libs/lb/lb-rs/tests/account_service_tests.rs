use lb_rs::CoreError;
use lockbook_shared::account::{Account, MAX_USERNAME_LENGTH};
use lockbook_shared::pubkey;
use test_utils::*;

#[test]
fn create_account_success() {
    let core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();
}

#[test]
fn create_account_success_with_welcome() {
    let core = test_core();
    core.create_account(&random_name(), &url(), true).unwrap();
    let welcome_doc = core.get_by_path("welcome.md").unwrap().id;
    assert!(String::from_utf8_lossy(&core.read_document(welcome_doc).unwrap())
        .to_lowercase()
        .contains("welcome"));
}

#[test]
fn create_account_invalid_url() {
    let core = test_core();
    let result = core.create_account(&random_name(), "https://bad-url.net", false);
    assert!(matches!(result.unwrap_err().kind, CoreError::ServerUnreachable))
}

#[test]
fn create_account_invalid_url_with_welcome() {
    let core = test_core();
    let result = core.create_account(&random_name(), "https://bad-url.net", true);
    assert!(matches!(result.unwrap_err().kind, CoreError::ServerUnreachable))
}

#[test]
fn create_account_username_taken() {
    let core1 = test_core();
    let core2 = test_core();
    let name = random_name();

    core1.create_account(&name, &url(), false).unwrap();

    let err = core2.create_account(&name, &url(), false).unwrap_err();

    assert!(
        matches!(err.kind, CoreError::UsernameTaken),
        "Username \"{}\" should have caused a UsernameTaken error but instead was {:?}",
        &name,
        err
    )
}

#[test]
fn create_account_invalid_username() {
    let core = test_core();

    let invalid_unames =
        ["", "i/o", "@me", "###", "+1", "💩", &"x".repeat(MAX_USERNAME_LENGTH + 1)];

    for &uname in &invalid_unames {
        let err = core.create_account(uname, &url(), false).unwrap_err();

        assert!(
            matches!(err.kind, CoreError::UsernameInvalid),
            "Username \"{}\" should have been InvalidUsername but instead was {:?}",
            uname,
            err
        )
    }
}

#[test]
fn create_account_account_exists() {
    let core = &test_core();

    core.create_account(&random_name(), &url(), false).unwrap();

    assert!(
        matches!(
            core.create_account(&random_name(), &url(), false)
                .unwrap_err()
                .kind,
            CoreError::AccountExists
        ),
        "This action should have failed with AccountAlreadyExists!",
    );
}

#[test]
fn create_account_account_exists_case() {
    let core = test_core();
    let name = random_name();

    core.create_account(&name, &url(), false).unwrap();

    let core = test_core();
    assert!(matches!(
        core.create_account(&(name.to_uppercase()), &url(), false)
            .unwrap_err()
            .kind,
        CoreError::UsernameTaken
    ));
}

#[test]
fn import_account_account_exists() {
    let core = test_core();

    core.create_account(&random_name(), &url(), false).unwrap();
    let account_string = core.export_account().unwrap();

    assert!(matches!(
        core.import_account(&account_string).unwrap_err().kind,
        CoreError::AccountExists
    ));
}

#[test]
fn import_account_corrupted() {
    let core = test_core();

    assert!(matches!(
        core.import_account("clearly a bad account string")
            .unwrap_err()
            .kind,
        CoreError::AccountStringCorrupted
    ));
}

#[test]
fn import_account_corrupted_base64() {
    let core = test_core();

    base64::decode("clearlyabadaccountstring").unwrap();
    assert!(matches!(
        core.import_account("clearlyabadaccountstring")
            .unwrap_err()
            .kind,
        CoreError::AccountStringCorrupted
    ));
}

#[test]
fn import_account_nonexistent() {
    let core1 = test_core();

    core1.create_account(&random_name(), &url(), false).unwrap();

    let core2 = test_core();
    let account =
        Account { api_url: url(), username: random_name(), private_key: pubkey::generate_key() };
    core2
        .in_tx(|s| {
            s.db.account.insert(account).unwrap();
            Ok(())
        })
        .unwrap();
    let account_string = core2.export_account().unwrap();

    let core3 = test_core();
    assert!(matches!(
        core3.import_account(&account_string).unwrap_err().kind,
        CoreError::AccountNonexistent
    ));
}

#[test]
fn import_account_public_key_mismatch() {
    let bad_account_string = {
        let core1 = test_core();
        let core2 = test_core();
        let account1 = core1.create_account(&random_name(), &url(), false).unwrap();
        let mut account2 = core2.create_account(&random_name(), &url(), false).unwrap();
        account2.username = account1.username;
        core2
            .in_tx(|s| {
                s.db.account.insert(account2).unwrap();
                Ok(())
            })
            .unwrap();
        core2.export_account().unwrap()
    };

    let core3 = test_core();

    assert!(matches!(
        core3.import_account(&bad_account_string).unwrap_err().kind,
        CoreError::UsernamePublicKeyMismatch
    ));
}

#[test]
fn export_account() {
    let core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();
    core.export_account().unwrap();
    core.export_account_qr().unwrap();
}

#[test]
fn nonzero_root_version() {
    let core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();
    assert!(core.get_root().unwrap().last_modified > 0);
}
