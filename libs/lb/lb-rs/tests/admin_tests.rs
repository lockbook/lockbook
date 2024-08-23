use lb_rs::logic::api::ServerIndex;
use lb_rs::logic::file::ShareMode;
use test_utils::*;

#[test]
#[ignore]
fn admin_disappear_test() {
    let admin_core = test_core();
    admin_core.create_account("admin1", &url(), false).unwrap();

    let customer_core = test_core_with_account();
    let test1 = customer_core.create_at_path("test1.md").unwrap();
    let test2 = customer_core.create_at_path("test2.md").unwrap();
    customer_core.sync(None).unwrap();

    let account_string = customer_core.export_account().unwrap();
    let customer_core_2 = test_core();
    customer_core_2.import_account(&account_string).unwrap();
    assert_eq!(customer_core_2.calculate_work().unwrap().work_units.len(), 3);

    admin_core.admin_disappear_file(test2.id).unwrap();

    let account_string = customer_core.export_account().unwrap();
    let customer_core_2 = test_core();
    customer_core_2.import_account(&account_string).unwrap();
    assert_eq!(customer_core_2.calculate_work().unwrap().work_units.len(), 2);
    customer_core_2.sync(None).unwrap();

    assert!(customer_core_2
        .list_metadatas()
        .unwrap()
        .iter()
        .any(|f| f.id == test1.id));
    assert!(!customer_core_2
        .list_metadatas()
        .unwrap()
        .iter()
        .any(|f| f.id == test2.id));
    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());
}

#[test]
#[ignore]
fn admin_disappear_file_shared_with_disappeared_account() {
    let admin_core = test_core();
    admin_core.create_account("admin1", &url(), false).unwrap();

    let customer1 = test_core_with_account();
    let customer2 = test_core_with_account();

    let doc = customer1.create_at_path("test.md").unwrap();
    customer1
        .share_file(doc.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .unwrap();
    customer1.sync(None).unwrap();
    customer2.sync(None).unwrap();

    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    admin_core
        .admin_disappear_account(&customer2.get_account().unwrap().username)
        .unwrap();
    admin_core.admin_disappear_file(doc.id).unwrap();

    customer1.sync(None).unwrap();
    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    let cust1_new_device = test_core_from(&customer1);
    cust1_new_device.validate().unwrap();
}

#[test]
#[ignore]
fn admin_disappear_folder_shared_with_disappeared_account() {
    let admin_core = test_core();
    admin_core.create_account("admin1", &url(), false).unwrap();

    let customer1 = test_core_with_account();
    let customer2 = test_core_with_account();

    let folder = customer1.create_at_path("folder/").unwrap();
    customer1.create_at_path("folder/test.md").unwrap();
    customer1
        .share_file(folder.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .unwrap();
    customer1.sync(None).unwrap();
    customer2.sync(None).unwrap();

    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    admin_core
        .admin_disappear_account(&customer2.get_account().unwrap().username)
        .unwrap();
    admin_core.admin_disappear_file(folder.id).unwrap();

    customer1.sync(None).unwrap();
    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    let cust1_new_device = test_core_from(&customer1);
    cust1_new_device.validate().unwrap();
}

#[test]
#[ignore]
fn admin_rebuild_owned_files_index_test() {
    let admin_core = test_core();
    admin_core.create_account("admin1", &url(), false).unwrap();

    let customer1 = test_core_with_account();
    let customer2 = test_core_with_account();

    let doc = customer1.create_at_path("test.md").unwrap();
    customer1
        .share_file(doc.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .unwrap();
    customer1.sync(None).unwrap();
    customer2.sync(None).unwrap();

    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    admin_core
        .admin_disappear_account(&customer2.get_account().unwrap().username)
        .unwrap();

    // this statement failed before fix of https://github.com/lockbook/lockbook/issues/1521
    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    admin_core
        .admin_rebuild_index(ServerIndex::OwnedFiles)
        .unwrap();
    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    let cust1_new_device = test_core_from(&customer1);
    cust1_new_device.validate().unwrap();
}

#[test]
#[ignore]
fn admin_rebuild_shared_files_index_test() {
    let admin_core = test_core();
    admin_core.create_account("admin1", &url(), false).unwrap();

    let customer1 = test_core_with_account();
    let customer2 = test_core_with_account();

    let doc = customer1.create_at_path("test.md").unwrap();
    customer1
        .share_file(doc.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .unwrap();
    customer1.sync(None).unwrap();
    customer2.sync(None).unwrap();

    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    admin_core
        .admin_disappear_account(&customer1.get_account().unwrap().username)
        .unwrap();

    // this statement failed before fix of https://github.com/lockbook/lockbook/issues/1521
    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    admin_core
        .admin_rebuild_index(ServerIndex::SharedFiles)
        .unwrap();
    assert!(admin_core
        .admin_validate_server()
        .unwrap()
        .users_with_validation_failures
        .is_empty());

    let cust2_new_device = test_core_from(&customer2);
    cust2_new_device.validate().unwrap();
}
