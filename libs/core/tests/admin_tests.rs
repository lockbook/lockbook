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
}
