use image::EncodableLayout;
use lockbook_core::{Core, CoreError};
use lockbook_shared::document_repo;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType::Folder;
use lockbook_shared::file_metadata::{FileType, METADATA_FEE};
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

    assert!(core.get_usage().unwrap().usages.is_empty(), "Returned non-empty usage!");

    core.sync(None).unwrap();
    let hmac = core
        .in_tx(|s| {
            Ok(s.db
                .base_metadata
                .data()
                .get(&file.id)
                .unwrap()
                .document_hmac()
                .cloned())
        })
        .unwrap();
    let local_encrypted = document_repo::get(&core.get_config().unwrap(), &file.id, hmac.as_ref())
        .unwrap()
        .value;

    assert_eq!(core.get_usage().unwrap().usages[0].file_id, file.id);
    assert_eq!(core.get_usage().unwrap().usages.len(), 1);
    assert_eq!(
        core.get_usage().unwrap().usages[0].size_bytes,
        local_encrypted.len() as u64 + METADATA_FEE
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

    assert_eq!(core.get_usage().unwrap().usages, vec![]);
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
    assert_eq!(usages.usages.len(), 3);
    core.delete_file(folder.id).unwrap();
    for usage in usages.usages {
        assert_ne!(usage.size_bytes, 0);
    }
    core.sync(None).unwrap();

    let hmac = core
        .in_tx(|s| {
            Ok(s.db
                .base_metadata
                .data()
                .get(&file.id)
                .unwrap()
                .document_hmac()
                .cloned())
        })
        .unwrap();

    document_repo::get(&core.get_config().unwrap(), &file.id, hmac.as_ref()).unwrap();

    let usage = core.get_usage().unwrap_or_else(|err| panic!("{:?}", err));

    assert_eq!(usage.usages.len(), 1);
}

#[test]
fn usage_new_files_have_no_size() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();
    core.create_file(&random_name(), root.id, FileType::Document)
        .unwrap();

    assert!(core.get_usage().unwrap().usages.is_empty(), "Returned non-empty usage!");

    core.sync(None).unwrap();

    let total_usage = core
        .get_usage()
        .unwrap()
        .usages
        .iter()
        .filter(|usage| usage.file_id != root.id)
        .map(|usage| usage.size_bytes)
        .sum::<u64>();

    assert_eq!(total_usage, 0);
}

#[test]
fn change_doc_over_data_cap() {
    let core: Core = test_core_with_account();
    let document = core.create_at_path("hello.md").unwrap();
    let free_tier_limit = 1024 * 1024;
    let content: Vec<u8> = (0..(free_tier_limit * 2))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document.id, content.as_bytes())
        .unwrap();

    let result = core.sync(None);

    assert_eq!(result.unwrap_err().kind, CoreError::UsageIsOverFreeTierDataCap);
}

#[test]
fn change_doc_under_data_cap() {
    let core = test_core_with_account();
    let document = core.create_at_path("hello.md").unwrap();
    let free_tier_limit = 1024.0 * 1024.0;
    let content: Vec<u8> = (0..((free_tier_limit * 0.8) as i64))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document.id, content.as_bytes())
        .unwrap();

    core.sync(None).unwrap();

    assert::all_paths(&core, &["/", "/hello.md"]);
}

#[test]
fn old_file_and_new_large_one() {
    let core = test_core_with_account();
    let document1 = core.create_at_path("document1.md").unwrap();
    let free_tier_limit = 1024.0 * 1024.0;
    let content: Vec<u8> = (0..((free_tier_limit * 0.8) as i64))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document1.id, content.as_bytes())
        .unwrap();

    core.sync(None).unwrap();

    let document2 = core.create_at_path("document2.md").unwrap();
    core.write_document(document2.id, content.as_bytes())
        .unwrap();

    let result = core.sync(None);

    assert_eq!(result.unwrap_err().kind, CoreError::UsageIsOverFreeTierDataCap);
}

// todo: figure out how to change the metadata fee during testing.
// #[test]
// fn upsert_meta_over_data_cap() {
//     let core: Core = test_core_with_account();
//     let free_tier_limit = 1024 * 1024 / METADATA_FEE;

//     for i in 0..(free_tier_limit + 10) {
//         core.create_at_path(format!("document{}.md", i).as_str())
//             .unwrap();
//     }

//     let result = core.sync(None);

//     assert_eq!(result.unwrap_err().kind, CoreError::UsageIsOverFreeTierDataCap);
// }
