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

#[test]
fn create_sync_compare() {
    let server = InProcess::init(test_config());
    let core1 = CoreIP::init_in_process(&test_config(), server.clone());
    let core2 = CoreIP::init_in_process(&test_config(), server.clone());
    core1.create_account(&random_name(), "unused af").unwrap();
    core2
        .import_account(&core1.export_account().unwrap())
        .unwrap();
    core2.sync(None).unwrap();

    let doc = core2.create_at_path("test.md").unwrap();
    core2.write_document(doc.id, b"test").unwrap();

    core1.sync(None).unwrap();
    core2.sync(None).unwrap();
    core1.sync(None).unwrap();
    core2.sync(None).unwrap();

    assert!(dbs_equal(&core1, &core2));

    println!("{}", server.config.writeable_path);
    println!("{}", core1.config.writeable_path);
    println!("{}", core2.config.writeable_path);
}
