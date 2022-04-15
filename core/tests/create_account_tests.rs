mod test_utils;

use crate::assert_matches;
use crate::test_utils::test_core_with_account;
use lockbook_core::pure_functions::files;
use lockbook_core::service::api_service::ApiError;
use lockbook_core::service::{api_service, file_encryption_service};
use lockbook_crypto::pubkey;
use lockbook_models::api::*;

// In addition to core, server should enforce username case-insensitivity
#[test]
fn create_account_username_case() {
    let core = test_core_with_account();
    let mut account = core.get_account().unwrap();

    account.username = account.username.to_uppercase();
    account.private_key = pubkey::generate_key();

    let root =
        &file_encryption_service::encrypt_metadata(&account, &[files::create_root(&account)])
            .unwrap()[0];

    assert_matches!(
        api_service::request(&account, NewAccountRequest::new(&account, &root)),
        Err(ApiError::Endpoint(NewAccountError::UsernameTaken))
    );
}
