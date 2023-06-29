use image::EncodableLayout;
use lockbook_core::{Core, CoreError, DocumentService, OnDiskDocuments, ShareMode};
use lockbook_shared::api::{FREE_TIER_USAGE_SIZE, METADATA_FEE};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::file_metadata::FileType::Folder;
use test_utils::*;

#[test]
fn report_usage() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let file = core
        .create_file(&random_name(), root.id, FileType::Document)
        .unwrap();
    core.write_document(file.id, "0000000000".as_bytes())
        .unwrap();

    assert!(
        core.get_usage().unwrap().usages.len() == 1,
        "Didn't account for the cost of root file metadata"
    );
    assert_eq!(core.get_usage().unwrap().usages[0].size_bytes, METADATA_FEE);

    core.sync(None).unwrap();
    let hmac = core
        .in_tx(|s| {
            Ok(s.db
                .base_metadata
                .get()
                .get(&file.id)
                .unwrap()
                .document_hmac()
                .cloned())
        })
        .unwrap();
    let docs = OnDiskDocuments::from(&core.get_config().unwrap());
    let local_encrypted = docs.get(&file.id, hmac.as_ref()).unwrap().value;

    assert_eq!(core.get_usage().unwrap().usages.len(), 2);
    assert_eq!(
        core.get_usage()
            .unwrap()
            .usages
            .iter()
            .map(|f| f.size_bytes)
            .sum::<u64>(),
        local_encrypted.len() as u64 + METADATA_FEE * 2
    )
}

#[test]
fn usage_go_back_down_after_delete() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let file = core
        .create_file(&random_name(), root.id, FileType::Document)
        .unwrap();
    core.write_document(file.id, &String::from("0000000000").into_bytes())
        .unwrap();

    core.sync(None).unwrap();
    core.delete_file(file.id).unwrap();
    core.sync(None).unwrap();

    assert_eq!(
        core.get_usage()
            .unwrap()
            .usages
            .iter()
            .map(|f| f.size_bytes)
            .sum::<u64>(),
        METADATA_FEE * 2
    );
}

#[test]
fn usage_go_back_down_after_delete_folder() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let folder = core.create_file("folder", root.id, Folder).unwrap();
    let file = core
        .create_file(&random_name(), root.id, FileType::Document)
        .unwrap();
    core.write_document(file.id, &String::from("0000000000").into_bytes())
        .unwrap();
    let file2 = core
        .create_file(&random_name(), folder.id, FileType::Document)
        .unwrap();
    core.write_document(file2.id, &String::from("0000000000").into_bytes())
        .unwrap();
    let file3 = core
        .create_file(&random_name(), folder.id, FileType::Document)
        .unwrap();
    core.write_document(file3.id, &String::from("0000000000").into_bytes())
        .unwrap();

    core.sync(None).unwrap();
    let usages = core.get_usage().unwrap();
    assert_eq!(usages.usages.len(), 5);
    core.delete_file(folder.id).unwrap();
    for usage in usages.usages {
        assert_ne!(usage.size_bytes, 0);
    }
    core.sync(None).unwrap();

    let hmac = core
        .in_tx(|s| {
            Ok(s.db
                .base_metadata
                .get()
                .get(&file.id)
                .unwrap()
                .document_hmac()
                .cloned())
        })
        .unwrap();

    let docs = OnDiskDocuments::from(&core.get_config().unwrap());
    docs.get(&file.id, hmac.as_ref()).unwrap();

    let usage = core.get_usage().unwrap_or_else(|err| panic!("{:?}", err));

    assert_eq!(usage.usages.len(), 5);
}

#[test]
fn usage_new_files_have_no_size() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();
    core.create_file(&random_name(), root.id, FileType::Document)
        .unwrap();

    assert!(
        core.get_usage().unwrap().usages.len() == 1,
        "Didn't account for the cost of root file metadata"
    );

    core.sync(None).unwrap();

    let total_usage = core
        .get_usage()
        .unwrap()
        .usages
        .iter()
        .filter(|usage| usage.file_id != root.id)
        .map(|usage| usage.size_bytes)
        .sum::<u64>();

    assert_eq!(total_usage, METADATA_FEE);
}

#[test]
fn change_doc_over_data_cap() {
    let core: Core = test_core_with_account();
    let document = core.create_at_path("hello.md").unwrap();
    let content: Vec<u8> = (0..(FREE_TIER_USAGE_SIZE - METADATA_FEE * 2))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document.id, content.as_bytes())
        .unwrap();

    let result = core.sync(None);

    assert_eq!(result.unwrap_err().kind, CoreError::UsageIsOverDataCap);
}

#[test]
fn old_file_and_new_large_one() {
    let core = test_core_with_account();
    let document1 = core.create_at_path("document1.md").unwrap();
    let content: Vec<u8> = (0..((FREE_TIER_USAGE_SIZE as f64 * 0.8) as i64))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document1.id, content.as_bytes())
        .unwrap();

    core.sync(None).unwrap();

    let document2 = core.create_at_path("document2.md").unwrap();
    core.write_document(document2.id, content.as_bytes())
        .unwrap();

    let result = core.sync(None);

    assert_eq!(result.unwrap_err().kind, CoreError::UsageIsOverDataCap);
}

#[test]
fn upsert_meta_over_data_cap() {
    let core: Core = test_core_with_account();

    let document = core.create_at_path("document.md").unwrap();

    let content: Vec<u8> = (0..(FREE_TIER_USAGE_SIZE - 5 * METADATA_FEE))
        .map(|_| rand::random::<u8>())
        .collect();

    core.write_document(document.id, content.as_bytes())
        .unwrap();

    core.sync(None).unwrap();

    let hmac = core
        .in_tx(|s| {
            Ok(s.db
                .base_metadata
                .get()
                .get(&document.id)
                .unwrap()
                .document_hmac()
                .cloned())
        })
        .unwrap();
    let docs = OnDiskDocuments::from(&core.get_config().unwrap());
    let local_encrypted = docs.get(&document.id, hmac.as_ref()).unwrap().value;

    let file_capacity =
        (FREE_TIER_USAGE_SIZE - local_encrypted.len() as u64) as f64 / METADATA_FEE as f64;

    for i in 0..file_capacity as i64 - 2 {
        core.create_at_path(format!("document{}.md", i).as_str())
            .unwrap();
        core.sync(None).unwrap();
    }

    core.create_at_path("the_file_that_broke_the_camel's_back.md")
        .unwrap();

    let result = core.sync(None);
    assert_eq!(result.unwrap_err().kind, CoreError::UsageIsOverDataCap);
}

#[test]
fn upsert_meta_empty_folder_over_data_cap() {
    let core: Core = test_core_with_account();
    let free_tier_limit = FREE_TIER_USAGE_SIZE / METADATA_FEE;
    let root = core.get_root().unwrap();

    for _ in 0..(free_tier_limit + 10) {
        core.create_file(&uuid::Uuid::new_v4().to_string(), root.id, FileType::Document)
            .unwrap();
    }
    let result = core.sync(None);

    assert_eq!(result.unwrap_err().kind, CoreError::UsageIsOverDataCap);
}

#[test]
fn shared_docs_excluded() {
    let cores: Vec<Core> = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();

    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder.id).unwrap();

    cores[1].sync(None).unwrap();

    assert_eq!(cores[1].get_usage().unwrap().usages.len(), 2);
}
