use image::EncodableLayout;
use itertools::Itertools;
use lockbook_core::{Core, CoreError};
use lockbook_shared::file::ShareMode;
use test_utils::*;

/// Uncategorized tests.

//todo: maybe these deserve a new home? usage_caps_tests.rs for example
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

#[test]
fn test_path_conflict() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.create_at_path("new.md").unwrap();
    db1.sync(None).unwrap();
    db2.create_at_path("new.md").unwrap();
    db2.sync(None).unwrap();

    assert_eq!(
        db2.list_metadatas()
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new.md"]
    )
}

#[test]
fn test_path_conflict2() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.create_at_path("new-1.md").unwrap();
    db1.sync(None).unwrap();
    db2.create_at_path("new-1.md").unwrap();
    db2.sync(None).unwrap();

    assert_eq!(
        db2.list_metadatas()
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new-2.md"]
    )
}

#[test]
fn deleted_path_is_released() {
    let db1 = test_core_with_account();
    let file1 = db1.create_at_path("file1.md").unwrap();
    db1.sync(None).unwrap();
    db1.delete_file(file1.id).unwrap();
    db1.sync(None).unwrap();
    db1.create_at_path("file1.md").unwrap();
    db1.sync(None).unwrap();

    let db2 = test_core_from(&db1);
    db2.sync(None).unwrap();
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_1() {
    let db1 = test_core_with_account();
    let b = db1.create_at_path("/b").unwrap();
    let c = db1.create_at_path("/c/").unwrap();
    let d = db1.create_at_path("/c/d/").unwrap();
    db1.move_file(b.id, d.id).unwrap();
    db1.move_file(c.id, d.id).unwrap_err();
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_2() {
    let db1 = test_core_with_account();
    let root = db1.get_root().unwrap();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let a = db2.create_at_path("/a/").unwrap();
    let b = db2.create_at_path("/a/b/").unwrap();
    db2.move_file(b.id, root.id).unwrap();
    db2.rename_file(b.id, "b2").unwrap();
    let _c = db2.create_at_path("/c/").unwrap();
    db2.move_file(b.id, a.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert::cores_equal(&db1, &db2);
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_3() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let _a = db2.create_at_path("/a/").unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert::cores_equal(&db1, &db2);

    db1.create_at_path("/a/b.md").unwrap();
    let c = db1.create_at_path("/a/c").unwrap();
    db1.rename_file(c.id, "c2").unwrap();

    db1.create_at_path("/a/d").unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert::cores_equal(&db1, &db2);
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_4() {
    let db1 = test_core_with_account();
    let root = db1.get_root().unwrap();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let _a = db2.create_at_path("/a/").unwrap();
    let b = db2.create_at_path("/a/b/").unwrap();
    db2.move_file(b.id, root.id).unwrap();
    db2.rename_file(b.id, "b2").unwrap();
    let c = db2.create_at_path("c.md").unwrap();
    db2.write_document(c.id, b"DPCN8G0CK8qXSyJhervmmEXFnkt")
        .unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert::cores_equal(&db1, &db2);
}

#[test]
fn fuzzer_stuck_test_5() {
    let db1 = test_core_with_account();
    let root = db1.get_root().unwrap();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let a = db1.create_at_path("/a/").unwrap();
    let b = db1.create_at_path("/a/b/").unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert::cores_equal(&db1, &db2);

    db1.move_file(b.id, root.id).unwrap();
    db1.move_file(a.id, b.id).unwrap();
    db1.delete_file(b.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert::cores_equal(&db1, &db2);
}

#[test]
fn fuzzer_stuck_test_6() {
    let core1 = test_core_with_account();

    let dir1 = core1.create_at_path("quB/").unwrap();
    let dir2 = core1.create_at_path("OO1/").unwrap();
    core1.sync(None).unwrap();
    let core2 = test_core_from(&core1);
    core2.move_file(dir2.id, dir1.id).unwrap();
    let _doc1 = core1.create_at_path("KbW").unwrap();
    core1.move_file(dir1.id, dir2.id).unwrap();

    core1.sync(None).unwrap();
    core2.sync(None).unwrap();
    core1.sync(None).unwrap();
    core2.sync(None).unwrap();
    core1.validate().unwrap();
    assert::cores_equal(&core1, &core2);
}

#[test]
fn fuzzer_get_updates_required_test() {
    let db1 = test_core_with_account();

    let document = db1.create_at_path("/document").unwrap();

    db1.sync(None).unwrap();
    let db2 = test_core_from(&db1);

    db1.write_document(document.id, b"document content")
        .unwrap();
    db2.write_document(document.id, b"content document")
        .unwrap();
    db2.delete_file(document.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
}

#[test]
fn fuzzer_new_file_deleted() {
    let core = test_core_with_account();

    let dir1 = core.create_at_path("u88/").unwrap();
    core.sync(None).unwrap();
    let dir2 = core.create_at_path("mep/").unwrap();
    core.move_file(dir1.id, dir2.id).unwrap();
    core.delete_file(dir2.id).unwrap();
    core.sync(None).unwrap();
}

#[test]
fn fuzzer_create_document_in_renamed_concurrently_deleted_folder() {
    let core1 = test_core_with_account();
    let core2 = test_core_from(&core1);

    let folder = core1.create_at_path("folder/").unwrap();

    core1.sync(None).unwrap();
    core2.sync(None).unwrap();

    core1.delete_file(folder.id).unwrap();
    core1.sync(None).unwrap();

    let document = core2.create_at_path("folder/document").unwrap();
    core2
        .write_document(document.id, b"document content")
        .unwrap();
    core2.rename_file(folder.id, "folder-renamed").unwrap();
    core2.sync(None).unwrap();
}

#[test]
fn fuzzer_delete_concurrently_edited_document() {
    let core1 = test_core_with_account();
    let core2 = test_core_from(&core1);

    let document = core1.create_at_path("document.md").unwrap();

    core1.sync(None).unwrap();
    core2.sync(None).unwrap();

    core1.write_document(document.id, b"content").unwrap();
    core1.sync(None).unwrap();

    core2.write_document(document.id, b"content").unwrap();
    core2.delete_file(document.id).unwrap();
    core2.sync(None).unwrap();
}

#[test]
fn test_move_folder_with_deleted_file() {
    let mut cores = vec![
        vec![test_core_with_account()],
        vec![test_core_with_account()],
        vec![test_core_with_account()],
    ];
    let c = another_client(&cores[1][0]);
    cores[1].push(c);
    let c = another_client(&cores[1][0]);
    cores[1].push(c);
    let c = another_client(&cores[2][0]);
    cores[2].push(c);

    let us6 = cores[0][0].create_at_path("US62E5M/").unwrap();
    let voe = cores[0][0].create_at_path("US62E5M/voey6qi.md").unwrap();
    cores[0][0].delete_file(voe.id).unwrap();
    let us7 = cores[0][0].create_at_path("US7/").unwrap();
    cores[0][0].move_file(us6.id, us7.id).unwrap();
}

#[test]
fn test_clean_sync_deleted_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];

    let doc = cores[0].create_at_path("welcome.md").unwrap();
    cores[0]
        .share_file(doc.id, &cores[1].get_account().unwrap().username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let link_doc = cores[1]
        .create_link_at_path("welcome-path.md", doc.id)
        .unwrap();
    cores[1].sync(None).unwrap();
    cores[1].delete_file(link_doc.id).unwrap();
    cores[1].delete_pending_share(doc.id).unwrap();
    cores[1].sync(None).unwrap();

    another_client(&cores[1]).sync(None).unwrap();
}

#[test]
fn test_unmergable_conflict_progress_closure() {
    let mut cores = vec![test_core_with_account()];
    cores.push(another_client(&cores[0]));

    let doc = cores[0].create_at_path("test.md").unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[0].write_document(doc.id, b"a").unwrap();
    cores[1].write_document(doc.id, b"b").unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(Some(Box::new(|_| {}))).unwrap();
}
