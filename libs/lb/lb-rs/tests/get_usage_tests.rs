use lb_rs::Lb;
use lb_rs::io::docs::AsyncDocs;
use lb_rs::model::api::{FREE_TIER_USAGE_SIZE, METADATA_FEE};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::file_metadata::FileType::Folder;
use test_utils::*;

#[tokio::test]
async fn report_usage() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();

    let file = core
        .create_file(&random_name(), &root.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(file.id, "0000000000".as_bytes())
        .await
        .unwrap();

    assert!(
        core.get_usage().await.unwrap().usages.len() == 1,
        "Didn't account for the cost of root file metadata"
    );
    assert_eq!(core.get_usage().await.unwrap().usages[0].size_bytes, METADATA_FEE);

    core.sync(None).await.unwrap();
    let hmac = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get()
        .get(&file.id)
        .unwrap()
        .document_hmac()
        .cloned();
    let docs = AsyncDocs::from(&core.config);
    let local_encrypted = docs.get(file.id, hmac).await.unwrap().value;

    assert_eq!(core.get_usage().await.unwrap().usages.len(), 2);
    assert_eq!(
        core.get_usage()
            .await
            .unwrap()
            .usages
            .iter()
            .map(|f| f.size_bytes)
            .sum::<u64>(),
        local_encrypted.len() as u64 + METADATA_FEE * 2
    )
}

#[tokio::test]
async fn usage_go_back_down_after_delete() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();

    let file = core
        .create_file(&random_name(), &root.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(file.id, &String::from("0000000000").into_bytes())
        .await
        .unwrap();

    core.sync(None).await.unwrap();
    core.delete(&file.id).await.unwrap();
    core.sync(None).await.unwrap();

    assert_eq!(
        core.get_usage()
            .await
            .unwrap()
            .usages
            .iter()
            .map(|f| f.size_bytes)
            .sum::<u64>(),
        METADATA_FEE * 2
    );
}

#[tokio::test]
async fn usage_go_back_down_after_delete_folder() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();

    let folder = core.create_file("folder", &root.id, Folder).await.unwrap();
    let file = core
        .create_file(&random_name(), &root.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(file.id, &String::from("0000000000").into_bytes())
        .await
        .unwrap();
    let file2 = core
        .create_file(&random_name(), &folder.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(file2.id, &String::from("0000000000").into_bytes())
        .await
        .unwrap();
    let file3 = core
        .create_file(&random_name(), &folder.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(file3.id, &String::from("0000000000").into_bytes())
        .await
        .unwrap();

    core.sync(None).await.unwrap();
    let usages = core.get_usage().await.unwrap();
    assert_eq!(usages.usages.len(), 5);
    core.delete(&folder.id).await.unwrap();
    for usage in usages.usages {
        assert_ne!(usage.size_bytes, 0);
    }
    core.sync(None).await.unwrap();

    let hmac = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get()
        .get(&file.id)
        .unwrap()
        .document_hmac()
        .cloned();

    let docs = AsyncDocs::from(&core.config);
    docs.get(file.id, hmac).await.unwrap();

    let usage = core
        .get_usage()
        .await
        .unwrap_or_else(|err| panic!("{err:?}"));

    assert_eq!(usage.usages.len(), 5);
}

#[tokio::test]
async fn usage_new_files_have_no_size() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    core.create_file(&random_name(), &root.id, FileType::Document)
        .await
        .unwrap();

    assert!(
        core.get_usage().await.unwrap().usages.len() == 1,
        "Didn't account for the cost of root file metadata"
    );

    core.sync(None).await.unwrap();

    let total_usage = core
        .get_usage()
        .await
        .unwrap()
        .usages
        .iter()
        .filter(|usage| usage.file_id != root.id)
        .map(|usage| usage.size_bytes)
        .sum::<u64>();

    assert_eq!(total_usage, METADATA_FEE);
}

#[tokio::test]
async fn change_doc_over_data_cap() {
    let core: Lb = test_core_with_account().await;
    let document = core.create_at_path("hello.md").await.unwrap();
    let content: Vec<u8> = (0..(FREE_TIER_USAGE_SIZE - METADATA_FEE * 2))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document.id, &content).await.unwrap();

    let result = core.sync(None).await;

    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

#[tokio::test]
async fn old_file_and_new_large_one() {
    let core = test_core_with_account().await;
    let document1 = core.create_at_path("document1.md").await.unwrap();
    let content: Vec<u8> = (0..((FREE_TIER_USAGE_SIZE as f64 * 0.8) as i64))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document1.id, &content).await.unwrap();

    core.sync(None).await.unwrap();

    let document2 = core.create_at_path("document2.md").await.unwrap();
    core.write_document(document2.id, &content).await.unwrap();

    let result = core.sync(None).await;

    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

#[tokio::test]
async fn upsert_meta_over_data_cap() {
    let core: Lb = test_core_with_account().await;

    let document = core.create_at_path("document.md").await.unwrap();

    let content: Vec<u8> = (0..(FREE_TIER_USAGE_SIZE - 5 * METADATA_FEE))
        .map(|_| rand::random::<u8>())
        .collect();

    core.write_document(document.id, &content).await.unwrap();

    core.sync(None).await.unwrap();

    let hmac = {
        core.ro_tx()
            .await
            .db()
            .base_metadata
            .get()
            .get(&document.id)
            .unwrap()
            .document_hmac()
            .cloned()
    };
    let docs = AsyncDocs::from(&core.config);
    let local_encrypted = docs.get(document.id, hmac).await.unwrap().value;

    let file_capacity =
        (FREE_TIER_USAGE_SIZE - local_encrypted.len() as u64) as f64 / METADATA_FEE as f64;

    for i in 0..file_capacity as i64 - 2 {
        core.create_at_path(format!("document{i}.md").as_str())
            .await
            .unwrap();
        core.sync(None).await.unwrap();
    }

    core.create_at_path("the_file_that_broke_the_camel's_back.md")
        .await
        .unwrap();

    let result = core.sync(None).await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

#[tokio::test]
#[ignore]
async fn upsert_meta_empty_folder_over_data_cap() {
    let core: Lb = test_core_with_account().await;
    let free_tier_limit = FREE_TIER_USAGE_SIZE / METADATA_FEE;
    let root = core.root().await.unwrap();

    for _ in 0..(free_tier_limit + 10) {
        core.create_file(&uuid::Uuid::new_v4().to_string(), &root.id, FileType::Document)
            .await
            .unwrap();
    }
    let result = core.sync(None);

    assert_eq!(result.await.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

#[tokio::test]
async fn shared_docs_excluded() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();

    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();

    assert_eq!(cores[1].get_usage().await.unwrap().usages.len(), 2);
}
