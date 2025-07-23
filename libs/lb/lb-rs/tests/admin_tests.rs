use lb_rs::model::api::ServerIndex;
use lb_rs::model::file::ShareMode;
use test_utils::*;

#[tokio::test]
#[ignore]
async fn admin_disappear_test() {
    let admin_core = test_core().await;
    admin_core
        .create_account("admin1", &url(), false)
        .await
        .unwrap();

    let customer_core = test_core_with_account().await;
    let test1 = customer_core.create_at_path("test1.md").await.unwrap();
    let test2 = customer_core.create_at_path("test2.md").await.unwrap();
    customer_core.sync(None).await.unwrap();

    let account_string = customer_core.export_account_private_key().unwrap();
    let customer_core_2 = test_core().await;
    customer_core_2
        .import_account(&account_string, Some(&url()))
        .await
        .unwrap();
    assert_eq!(
        customer_core_2
            .calculate_work()
            .await
            .unwrap()
            .work_units
            .len(),
        3
    );

    admin_core.disappear_file(test2.id).await.unwrap();

    let account_string = customer_core.export_account_private_key().unwrap();
    let customer_core_2 = test_core().await;
    customer_core_2
        .import_account(&account_string, Some(&url()))
        .await
        .unwrap();
    assert_eq!(
        customer_core_2
            .calculate_work()
            .await
            .unwrap()
            .work_units
            .len(),
        2
    );
    customer_core_2.sync(None).await.unwrap();

    assert!(
        customer_core_2
            .list_metadatas()
            .await
            .unwrap()
            .iter()
            .any(|f| f.id == test1.id)
    );
    assert!(
        !customer_core_2
            .list_metadatas()
            .await
            .unwrap()
            .iter()
            .any(|f| f.id == test2.id)
    );
    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );
}

#[tokio::test]
#[ignore]
async fn disappear_file_shared_with_disappeared_account() {
    let admin_core = test_core().await;
    admin_core
        .create_account("admin1", &url(), false)
        .await
        .unwrap();

    let customer1 = test_core_with_account().await;
    let customer2 = test_core_with_account().await;

    let doc = customer1.create_at_path("test.md").await.unwrap();
    customer1
        .share_file(doc.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .await
        .unwrap();
    customer1.sync(None).await.unwrap();
    customer2.sync(None).await.unwrap();

    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    admin_core
        .disappear_account(&customer2.get_account().unwrap().username)
        .await
        .unwrap();
    admin_core.disappear_file(doc.id).await.unwrap();

    customer1.sync(None).await.unwrap();
    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    let cust1_new_device = test_core_from(&customer1).await;
    cust1_new_device.test_repo_integrity().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn admin_disappear_folder_shared_with_disappeared_account() {
    let admin_core = test_core().await;
    admin_core
        .create_account("admin1", &url(), false)
        .await
        .unwrap();

    let customer1 = test_core_with_account().await;
    let customer2 = test_core_with_account().await;

    let folder = customer1.create_at_path("folder/").await.unwrap();
    customer1.create_at_path("folder/test.md").await.unwrap();
    customer1
        .share_file(folder.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .await
        .unwrap();
    customer1.sync(None).await.unwrap();
    customer2.sync(None).await.unwrap();

    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    admin_core
        .disappear_account(&customer2.get_account().unwrap().username)
        .await
        .unwrap();
    admin_core.disappear_file(folder.id).await.unwrap();

    customer1.sync(None).await.unwrap();
    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    let cust1_new_device = test_core_from(&customer1).await;
    cust1_new_device.test_repo_integrity().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn admin_rebuild_owned_files_index_test() {
    let admin_core = test_core().await;
    admin_core
        .create_account("admin1", &url(), false)
        .await
        .unwrap();

    let customer1 = test_core_with_account().await;
    let customer2 = test_core_with_account().await;

    let doc = customer1.create_at_path("test.md").await.unwrap();
    customer1
        .share_file(doc.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .await
        .unwrap();
    customer1.sync(None).await.unwrap();
    customer2.sync(None).await.unwrap();

    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    admin_core
        .disappear_account(&customer2.get_account().unwrap().username)
        .await
        .unwrap();

    // this statement failed before fix of https://github.com/lockbook/lockbook/issues/1521
    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    admin_core
        .rebuild_index(ServerIndex::OwnedFiles)
        .await
        .unwrap();
    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    let cust1_new_device = test_core_from(&customer1).await;
    cust1_new_device.test_repo_integrity().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn admin_rebuild_shared_files_index_test() {
    let admin_core = test_core().await;
    admin_core
        .create_account("admin1", &url(), false)
        .await
        .unwrap();

    let customer1 = test_core_with_account().await;
    let customer2 = test_core_with_account().await;

    let doc = customer1.create_at_path("test.md").await.unwrap();
    customer1
        .share_file(doc.id, &customer2.get_account().unwrap().username, ShareMode::Read)
        .await
        .unwrap();
    customer1.sync(None).await.unwrap();
    customer2.sync(None).await.unwrap();

    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    admin_core
        .disappear_account(&customer1.get_account().unwrap().username)
        .await
        .unwrap();

    // this statement failed before fix of https://github.com/lockbook/lockbook/issues/1521
    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    admin_core
        .rebuild_index(ServerIndex::SharedFiles)
        .await
        .unwrap();
    assert!(
        admin_core
            .validate_server()
            .await
            .unwrap()
            .users_with_validation_failures
            .is_empty()
    );

    let cust2_new_device = test_core_from(&customer2).await;
    cust2_new_device.test_repo_integrity().await.unwrap();
}
