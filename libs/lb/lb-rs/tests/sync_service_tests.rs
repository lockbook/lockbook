use itertools::Itertools;
use lb_rs::model::file::ShareMode;
use test_utils::*;

/// Uncategorized tests.

#[tokio::test]
async fn test_path_conflict() {
    let db1 = test_core_with_account().await;
    let db2 = test_core_from(&db1).await;

    db1.create_at_path("new.md").await.unwrap();
    db1.sync().await.unwrap();
    db2.create_at_path("new.md").await.unwrap();
    db2.sync().await.unwrap();

    assert_eq!(
        db2.list_metadatas()
            .await
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new.md"]
    )
}

#[tokio::test]
async fn test_path_conflict2() {
    let db1 = test_core_with_account().await;
    let db2 = test_core_from(&db1).await;

    db1.create_at_path("new-1.md").await.unwrap();
    db1.sync().await.unwrap();
    db2.create_at_path("new-1.md").await.unwrap();
    db2.sync().await.unwrap();

    assert_eq!(
        db2.list_metadatas()
            .await
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new-2.md"]
    )
}

#[tokio::test]
async fn deleted_path_is_released() {
    let db1 = test_core_with_account().await;
    let file1 = db1.create_at_path("file1.md").await.unwrap();
    db1.sync().await.unwrap();
    db1.delete(&file1.id).await.unwrap();
    db1.sync().await.unwrap();
    db1.create_at_path("file1.md").await.unwrap();
    db1.sync().await.unwrap();

    let db2 = test_core_from(&db1).await;
    db2.sync().await.unwrap();
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[tokio::test]
async fn fuzzer_stuck_test_1() {
    let db1 = test_core_with_account().await;
    let b = db1.create_at_path("/b").await.unwrap();
    let c = db1.create_at_path("/c/").await.unwrap();
    let d = db1.create_at_path("/c/d/").await.unwrap();
    db1.move_file(&b.id, &d.id).await.unwrap();
    db1.move_file(&c.id, &d.id).await.unwrap_err();
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[tokio::test]
async fn fuzzer_stuck_test_2() {
    let db1 = test_core_with_account().await;
    let root = db1.root().await.unwrap();
    let db2 = test_core_from(&db1).await;

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();

    let a = db2.create_at_path("/a/").await.unwrap();
    let b = db2.create_at_path("/a/b/").await.unwrap();
    db2.move_file(&b.id, &root.id).await.unwrap();
    db2.rename_file(&b.id, "b2").await.unwrap();
    let _c = db2.create_at_path("/c/").await.unwrap();
    db2.move_file(&b.id, &a.id).await.unwrap();

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(&db1, &db2).await;
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[tokio::test]
async fn fuzzer_stuck_test_3() {
    let db1 = test_core_with_account().await;
    let db2 = test_core_from(&db1).await;

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();

    let _a = db2.create_at_path("/a/").await.unwrap();

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(&db1, &db2).await;

    db1.create_at_path("/a/b.md").await.unwrap();
    let c = db1.create_at_path("/a/c").await.unwrap();
    db1.rename_file(&c.id, "c2").await.unwrap();

    db1.create_at_path("/a/d").await.unwrap();

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(&db1, &db2).await;
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[tokio::test]
async fn fuzzer_stuck_test_4() {
    let db1 = test_core_with_account().await;
    let root = db1.root().await.unwrap();
    let db2 = test_core_from(&db1).await;

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();

    let _a = db2.create_at_path("/a/").await.unwrap();
    let b = db2.create_at_path("/a/b/").await.unwrap();
    db2.move_file(&b.id, &root.id).await.unwrap();
    db2.rename_file(&b.id, "b2").await.unwrap();
    let c = db2.create_at_path("c.md").await.unwrap();
    db2.write_document(c.id, b"DPCN8G0CK8qXSyJhervmmEXFnkt")
        .await
        .unwrap();

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(&db1, &db2).await;
}

#[tokio::test]
async fn fuzzer_stuck_test_5() {
    let db1 = test_core_with_account().await;
    let root = db1.root().await.unwrap();
    let db2 = test_core_from(&db1).await;

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();

    let a = db1.create_at_path("/a/").await.unwrap();
    let b = db1.create_at_path("/a/b/").await.unwrap();

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(&db1, &db2).await;

    db1.move_file(&b.id, &root.id).await.unwrap();
    db1.move_file(&a.id, &b.id).await.unwrap();
    db1.delete(&b.id).await.unwrap();

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(&db1, &db2).await;
}

#[tokio::test]
async fn fuzzer_stuck_test_6() {
    let core1 = test_core_with_account().await;

    let dir1 = core1.create_at_path("quB/").await.unwrap();
    let dir2 = core1.create_at_path("OO1/").await.unwrap();
    core1.sync().await.unwrap();
    let core2 = test_core_from(&core1).await;
    core2.move_file(&dir2.id, &dir1.id).await.unwrap();
    let _doc1 = core1.create_at_path("KbW").await.unwrap();
    core1.move_file(&dir1.id, &dir2.id).await.unwrap();

    core1.sync().await.unwrap();
    core2.sync().await.unwrap();
    core1.sync().await.unwrap();
    core2.sync().await.unwrap();
    core1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(&core1, &core2).await;
}

#[tokio::test]
async fn fuzzer_get_updates_required_test() {
    let db1 = test_core_with_account().await;

    let document = db1.create_at_path("/document").await.unwrap();

    db1.sync().await.unwrap();
    let db2 = test_core_from(&db1).await;

    db1.write_document(document.id, b"document content")
        .await
        .unwrap();
    db2.write_document(document.id, b"content document")
        .await
        .unwrap();
    db2.delete(&document.id).await.unwrap();

    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
    db1.sync().await.unwrap();
    db2.sync().await.unwrap();
}

#[tokio::test]
async fn fuzzer_new_file_deleted() {
    let core = test_core_with_account().await;

    let dir1 = core.create_at_path("u88/").await.unwrap();
    core.sync().await.unwrap();
    let dir2 = core.create_at_path("mep/").await.unwrap();
    core.move_file(&dir1.id, &dir2.id).await.unwrap();
    core.delete(&dir2.id).await.unwrap();
    core.sync().await.unwrap();
}

#[tokio::test]
async fn fuzzer_create_document_in_renamed_concurrently_deleted_folder() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_from(&core1).await;

    let folder = core1.create_at_path("folder/").await.unwrap();

    core1.sync().await.unwrap();
    core2.sync().await.unwrap();

    core1.delete(&folder.id).await.unwrap();
    core1.sync().await.unwrap();

    let document = core2.create_at_path("folder/document").await.unwrap();
    core2
        .write_document(document.id, b"document content")
        .await
        .unwrap();
    core2
        .rename_file(&folder.id, "folder-renamed")
        .await
        .unwrap();
    core2.sync().await.unwrap();
}

#[tokio::test]
async fn fuzzer_delete_concurrently_edited_document() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_from(&core1).await;

    let document = core1.create_at_path("document.md").await.unwrap();

    core1.sync().await.unwrap();
    core2.sync().await.unwrap();

    core1.write_document(document.id, b"content").await.unwrap();
    core1.sync().await.unwrap();

    core2.write_document(document.id, b"content").await.unwrap();
    core2.delete(&document.id).await.unwrap();
    core2.sync().await.unwrap();
}

#[tokio::test]
async fn test_move_folder_with_deleted_file() {
    let mut cores = [
        vec![test_core_with_account().await],
        vec![test_core_with_account().await],
        vec![test_core_with_account().await],
    ];
    let c = another_client(&cores[1][0]).await;
    cores[1].push(c);
    let c = another_client(&cores[1][0]).await;
    cores[1].push(c);
    let c = another_client(&cores[2][0]).await;
    cores[2].push(c);

    let us6 = cores[0][0].create_at_path("US62E5M/").await.unwrap();
    let voe = cores[0][0]
        .create_at_path("US62E5M/voey6qi.md")
        .await
        .unwrap();
    cores[0][0].delete(&voe.id).await.unwrap();
    let us7 = cores[0][0].create_at_path("US7/").await.unwrap();
    cores[0][0].move_file(&us6.id, &us7.id).await.unwrap();
}

#[tokio::test]
async fn test_clean_sync_deleted_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];

    let doc = cores[0].create_at_path("welcome.md").await.unwrap();
    cores[0]
        .share_file(doc.id, &cores[1].get_account().unwrap().username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    let link_doc = cores[1]
        .create_link_at_path("welcome-path.md", doc.id)
        .await
        .unwrap();
    cores[1].sync().await.unwrap();
    cores[1].delete(&link_doc.id).await.unwrap();
    cores[1].reject_share(&doc.id).await.unwrap();
    cores[1].sync().await.unwrap();

    another_client(&cores[1]).await.sync().await.unwrap();
}

#[tokio::test]
async fn test_unmergable_conflict_progress_closure() {
    let mut cores = vec![test_core_with_account().await];
    let new_core = another_client(&cores[0]).await;
    cores.push(new_core);

    let doc = cores[0].create_at_path("test.md").await.unwrap();

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    cores[0].write_document(doc.id, b"a").await.unwrap();
    cores[1].write_document(doc.id, b"b").await.unwrap();

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();
}

#[tokio::test]
async fn concurrent_chat_appends_union_cleanly() {
    let c1 = test_core_with_account().await;
    let doc = c1.create_at_path("convo.chat").await.unwrap();

    let base = "{\"from\":\"a\",\"content\":\"hello\",\"ts\":1}\n";
    c1.write_document(doc.id, base.as_bytes()).await.unwrap();
    c1.sync().await.unwrap();

    let c2 = test_core_from(&c1).await;

    // each device appends its own turn on top of the shared base
    let turn1 = format!("{base}{{\"from\":\"a\",\"content\":\"one\",\"ts\":2}}\n");
    let turn2 = format!("{base}{{\"from\":\"b\",\"content\":\"two\",\"ts\":3}}\n");
    c1.write_document(doc.id, turn1.as_bytes()).await.unwrap();
    c2.write_document(doc.id, turn2.as_bytes()).await.unwrap();

    c1.sync().await.unwrap();
    c2.sync().await.unwrap();
    c1.sync().await.unwrap();
    c2.sync().await.unwrap();

    // no conflict copy: the single .chat is still the only document
    for c in [&c1, &c2] {
        let chats = c
            .list_metadatas()
            .await
            .unwrap()
            .into_iter()
            .filter(|f| f.name.ends_with(".chat"))
            .count();
        assert_eq!(chats, 1, "expected line-union, found a conflict copy");

        let merged = String::from_utf8(c.read_document(doc.id, false).await.unwrap()).unwrap();
        assert_eq!(merged.matches("\"content\":\"one\"").count(), 1);
        assert_eq!(merged.matches("\"content\":\"two\"").count(), 1);
        assert_eq!(merged.matches("\"content\":\"hello\"").count(), 1);
    }
    assert::cores_equal(&c1, &c2).await;
}
