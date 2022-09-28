use lockbook_core::model::errors::Error::UiError;
use lockbook_core::model::errors::*;
use lockbook_core::service::api_service::no_network::{CoreIP, InProcess};
use test_utils::test_config;
use test_utils::*;

#[test]
fn with_init_username_taken() {
    let server = InProcess::init(test_config());
    let core1 = CoreIP::init_in_process(&test_config(), server.clone());
    let core2 = CoreIP::init_in_process(&test_config(), server);
    let name = random_name();
    core1.create_account(&name, "not used").unwrap();
    assert_matches!(
        core2.create_account(&name, "not used"),
        Err(UiError(CreateAccountError::UsernameTaken))
    );
}
