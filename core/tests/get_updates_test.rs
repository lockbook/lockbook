use lockbook_core::service::api_service;
use lockbook_models::api::GetUpdatesRequest;
use test_utils::test_core_with_account;

#[test]
fn get_updates() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    // get updates at version 0
    let result = api_service::request(
        &core.client,
        &account,
        GetUpdatesRequest { since_metadata_version: 0 },
    )
    .unwrap()
    .file_metadata
    .len();
    assert_eq!(result, 1);

    // get updates at version of root folder
    let result = api_service::request(
        &core.client,
        &account,
        GetUpdatesRequest { since_metadata_version: root.metadata_version },
    )
    .unwrap()
    .file_metadata
    .len();
    assert_eq!(result, 0);
}
