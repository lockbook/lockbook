use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::*;
use test_utils::*;

#[test]
fn get_public_key() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let result =
        api_service::request(&account, GetPublicKeyRequest { username: account.username.clone() })
            .unwrap()
            .key;
    assert_eq!(result, account.public_key());
}

#[test]
fn get_public_key_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let result = api_service::request(&account, GetPublicKeyRequest { username: random_name() });
    assert_matches!(
        result,
        Err(ApiError::<GetPublicKeyError>::Endpoint(GetPublicKeyError::UserNotFound))
    );
}
