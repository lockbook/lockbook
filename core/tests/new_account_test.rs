extern crate lockbook_core;
use lockbook_core::lockbook_api::{new_account, NewAccountError, NewAccountParams};

fn api_loc() -> &'static str {
    env!("LOCKBOOK_API_LOCATION")
}

#[test]
fn test_create_user() -> Result<(), NewAccountError> {
    new_account(
        api_loc(),
        &NewAccountParams {
            username: "test_username".to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )
}
