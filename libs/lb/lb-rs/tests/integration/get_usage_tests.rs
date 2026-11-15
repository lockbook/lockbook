use lb_rs::io::docs::DocumentStore;
use lb_rs::io::server::Server;
use lb_rs::model::api::{FREE_TIER_USAGE_SIZE, GetUsageRequest, GetUsageResponse, METADATA_FEE};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::file_metadata::FileType::Folder;
use std::time::Duration;
use test_utils::*;
use uuid::Uuid;

async fn server_usage(core: &Lb) -> GetUsageResponse {
    let account = core.get_account().unwrap();
    Server::get_usage(&core.client, account, GetUsageRequest {})
        .await
        .unwrap()
}

async fn assert_synced_matches_server(core: &Lb) {
    let server = server_usage(core).await;
    let mut on_server: Vec<(Uuid, u64)> = server
        .usages
        .iter()
        .map(|usage| (usage.file_id, usage.size_bytes))
        .collect();
    let mut synced: Vec<(Uuid, u64)> = core
        .get_usage_breakdown()
        .await
        .unwrap()
        .into_iter()
        .filter(|entry| entry.synced > 0)
        .map(|entry| (entry.id, entry.synced))
        .collect();
    on_server.sort();
    synced.sort();

    assert_eq!(synced, on_server);
    assert_eq!(core.get_usage().await.unwrap().synced.exact, server.sum_server_usage());
}

async fn local_doc_size(core: &Lb, id: Uuid) -> u64 {
    core.ro_tx()
        .await
        .db()
        .local_metadata
        .get(&id)
        .unwrap()
        .doc_size()
        .unwrap() as u64
}

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

    let before = core.get_usage().await.unwrap();
    assert_eq!(
        before.synced.exact, METADATA_FEE,
        "Didn't account for the cost of root file metadata"
    );
    assert_eq!(before.added.exact, METADATA_FEE + local_doc_size(&core, file.id).await);
    assert_eq!(before.freed.exact, 0);
    assert_eq!(before.local.exact, before.synced.exact + before.added.exact);
    assert_eq!(server_usage(&core).await.sum_server_usage(), METADATA_FEE);

    core.sync().await.unwrap();
    let hmac = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get(&file.id)
        .unwrap()
        .document_hmac()
        .cloned();
    let docs = &core.docs;
    let local_encrypted = docs.get(file.id, hmac.unwrap()).await.unwrap().value;

    let after = core.get_usage().await.unwrap();
    assert_eq!(after.synced.exact, local_encrypted.len() as u64 + METADATA_FEE * 2);
    assert_eq!(after.synced.exact, before.local.exact);
    assert_eq!(after.local.exact, after.synced.exact);
    assert_eq!(after.added.exact, 0);
    assert_eq!(core.get_usage_breakdown().await.unwrap().len(), 2);
    assert_synced_matches_server(&core).await;
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

    core.sync().await.unwrap();
    let synced = core.get_usage().await.unwrap();
    core.delete(&file.id).await.unwrap();

    let deleted = core.get_usage().await.unwrap();
    assert_eq!(deleted.synced, synced.synced);
    assert_eq!(deleted.freed.exact, synced.synced.exact - METADATA_FEE);
    assert_eq!(deleted.local.exact, METADATA_FEE);

    core.sync().await.unwrap();

    let usage = core.get_usage().await.unwrap();
    assert_eq!(usage.synced.exact, METADATA_FEE);
    assert_eq!(usage.freed.exact, 0);
    assert_synced_matches_server(&core).await;
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

    core.sync().await.unwrap();
    let entries = core.get_usage_breakdown().await.unwrap();
    assert_eq!(entries.len(), 5);
    for entry in &entries {
        assert_ne!(entry.synced, 0);
        assert_eq!(entry.synced, entry.local);
    }

    core.delete(&folder.id).await.unwrap();
    let entries = core.get_usage_breakdown().await.unwrap();
    let freed: Vec<Uuid> = entries
        .iter()
        .filter(|entry| entry.local == 0)
        .map(|entry| entry.id)
        .collect();
    assert_eq!(freed.len(), 3);
    assert!(freed.contains(&folder.id));
    assert!(freed.contains(&file2.id));
    assert!(freed.contains(&file3.id));

    core.sync().await.unwrap();

    let hmac = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get(&file.id)
        .unwrap()
        .document_hmac()
        .cloned();

    let docs = &core.docs;
    docs.get(file.id, hmac.unwrap()).await.unwrap();

    assert_eq!(core.get_usage_breakdown().await.unwrap().len(), 2);
    assert_eq!(server_usage(&core).await.usages.len(), 2);
    assert_synced_matches_server(&core).await;
}

#[tokio::test]
async fn usage_new_files_have_no_size() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    core.create_file(&random_name(), &root.id, FileType::Document)
        .await
        .unwrap();

    assert_eq!(
        core.get_usage().await.unwrap().synced.exact,
        METADATA_FEE,
        "Didn't account for the cost of root file metadata"
    );

    core.sync().await.unwrap();

    let total_usage = core
        .get_usage_breakdown()
        .await
        .unwrap()
        .iter()
        .filter(|entry| entry.id != root.id)
        .map(|entry| entry.synced)
        .sum::<u64>();

    assert_eq!(total_usage, METADATA_FEE);
    assert_synced_matches_server(&core).await;
}

#[tokio::test]
async fn change_doc_over_data_cap() {
    let core: Lb = test_core_with_account().await;
    let document = core.create_at_path("hello.md").await.unwrap();
    let content: Vec<u8> = (0..(FREE_TIER_USAGE_SIZE - METADATA_FEE * 2))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document.id, &content).await.unwrap();

    let result = core.sync().await;

    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
    assert!(core.refresh_usage().await.unwrap().over_cap());
}

#[tokio::test]
async fn old_file_and_new_large_one() {
    let core = test_core_with_account().await;
    let document1 = core.create_at_path("document1.md").await.unwrap();
    let content: Vec<u8> = (0..((FREE_TIER_USAGE_SIZE as f64 * 0.8) as i64))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(document1.id, &content).await.unwrap();

    core.sync().await.unwrap();

    let document2 = core.create_at_path("document2.md").await.unwrap();
    core.write_document(document2.id, &content).await.unwrap();

    let result = core.sync().await;

    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

#[tokio::test]
async fn upsert_meta_over_data_cap() {
    let core: Lb = test_core_with_account().await;

    let document = core.create_at_path("document.md").await.unwrap();

    let content: Vec<u8> = (0..(FREE_TIER_USAGE_SIZE - 10 * METADATA_FEE))
        .map(|_| rand::random::<u8>())
        .collect();

    core.write_document(document.id, &content).await.unwrap();

    core.sync().await.unwrap();

    let hmac = {
        core.ro_tx()
            .await
            .db()
            .base_metadata
            .get(&document.id)
            .unwrap()
            .document_hmac()
            .cloned()
    };
    let docs = &core.docs;
    let local_encrypted = docs.get(document.id, hmac.unwrap()).await.unwrap().value;

    let file_capacity =
        (FREE_TIER_USAGE_SIZE - local_encrypted.len() as u64) as f64 / METADATA_FEE as f64;

    for i in 0..file_capacity as i64 - 2 {
        core.create_at_path(format!("document{i}.md").as_str())
            .await
            .unwrap();
        core.sync().await.unwrap();
    }

    core.create_at_path("the_file_that_broke_the_camel's_back.md")
        .await
        .unwrap();

    let result = core.sync().await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

/// After 25x'ing our free tier this test runs too slow to run repeatedely
/// creating 25k files takes a long amount of time, although each individual file seems
/// to take an acceptable amount of time (less than 50ms in a heavily instrumented env.)
/// It seems to be O(n) not anything worse.
///
/// for now, I think ignoring this test is acceptable because someone interacting with lockbook
/// normally will have a fine time even if they have 25k files. But it could be nice to improve
/// this for bulk import reasons and here are some possibilities:
///
/// - scrutinize link strictness, maybe also for UX reasons
/// - populate caches incrementally
/// - consider only performing validations on the staged side of a tree
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
    let result = core.sync();

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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    cores[1].sync().await.unwrap();

    assert_eq!(cores[1].get_usage_breakdown().await.unwrap().len(), 2);
    assert_synced_matches_server(&cores[1]).await;
}

#[tokio::test]
async fn sharee_cannot_exceed_sharer_data_cap() {
    let core_a = test_core_with_account().await;
    let core_b = test_core_with_account().await;
    let account_b = core_b.get_account().unwrap();

    // A shares a folder with B
    let folder = core_a.create_at_path("shared_folder/").await.unwrap();
    core_a
        .share_file(folder.id, &account_b.username, ShareMode::Write)
        .await
        .unwrap();
    core_a.sync().await.unwrap();

    // B syncs and creates a link to the shared folder
    core_b.sync().await.unwrap();
    core_b.create_link_at_path("link", folder.id).await.unwrap();
    core_b.sync().await.unwrap();

    // B creates 5 documents in the shared folder, each 1/4th of free tier usage
    // The 5th file should exceed A's data cap
    let file_size = FREE_TIER_USAGE_SIZE / 4;
    for i in 0..5 {
        let doc = core_b
            .create_file(&format!("doc{i}.md"), &folder.id, FileType::Document)
            .await
            .unwrap();
        let content: Vec<u8> = (0..file_size).map(|_| rand::random::<u8>()).collect();
        core_b.write_document(doc.id, &content).await.unwrap();
    }

    // B's sync should fail because the content would exceed A's data cap
    let result = core_b.sync().await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

#[tokio::test]
async fn sharee_cannot_exceed_sharer_data_cap_via_metadata() {
    let core_a = test_core_with_account().await;
    let core_b = test_core_with_account().await;
    let account_b = core_b.get_account().unwrap();

    // A shares a folder with B
    let folder = core_a.create_at_path("shared_folder/").await.unwrap();
    core_a
        .share_file(folder.id, &account_b.username, ShareMode::Write)
        .await
        .unwrap();
    core_a.sync().await.unwrap();

    // B syncs and creates a link to the shared folder
    core_b.sync().await.unwrap();
    core_b.create_link_at_path("link", folder.id).await.unwrap();
    core_b.sync().await.unwrap();

    // B creates a large document that uses most of A's data cap
    // Account for: root (1) + shared_folder (1) + large.md (1) = 3 files of metadata already
    // Leave room for ~5 more files of metadata
    let large_doc = core_b
        .create_file("large.md", &folder.id, FileType::Document)
        .await
        .unwrap();
    let content_size = FREE_TIER_USAGE_SIZE - (METADATA_FEE * 8);
    let content: Vec<u8> = (0..content_size).map(|_| rand::random::<u8>()).collect();
    core_b.write_document(large_doc.id, &content).await.unwrap();
    core_b.sync().await.unwrap();

    // Now B creates more empty files to exceed A's cap via metadata alone
    for i in 0..10 {
        core_b
            .create_file(&format!("file{i}.md"), &folder.id, FileType::Document)
            .await
            .unwrap();
    }

    // B's sync should fail because the metadata would exceed A's data cap
    let result = core_b.sync().await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);
}

#[tokio::test]
async fn cap_comes_from_refresh() {
    let core = test_core_with_account().await;

    let usage = core.refresh_usage().await.unwrap();
    assert_eq!(usage.data_cap.as_ref().unwrap().exact, FREE_TIER_USAGE_SIZE);
    assert!(!usage.over_cap());
    assert_eq!(core.get_usage().await.unwrap().data_cap.unwrap().exact, FREE_TIER_USAGE_SIZE);
}

#[tokio::test]
async fn synced_matches_server_across_shares() {
    let a = test_core_with_account().await;
    let b = test_core_with_account().await;
    let c = test_core_with_account().await;
    let b_name = b.get_account().unwrap().username.clone();

    let shared = a.create_at_path("shared/").await.unwrap();
    let doc = a.create_at_path("shared/a.md").await.unwrap();
    a.write_document(doc.id, b"from a").await.unwrap();
    let gone = a.create_at_path("gone/inner.md").await.unwrap();
    a.write_document(gone.id, b"deleted later").await.unwrap();
    a.share_file(shared.id, &b_name, ShareMode::Write)
        .await
        .unwrap();
    a.sync().await.unwrap();
    let gone_folder = a.get_file_by_id(gone.id).await.unwrap().parent;
    a.delete(&gone_folder).await.unwrap();
    a.sync().await.unwrap();

    let pending = c.create_at_path("pending/").await.unwrap();
    c.share_file(pending.id, &b_name, ShareMode::Read)
        .await
        .unwrap();
    c.sync().await.unwrap();

    b.sync().await.unwrap();
    b.create_link_at_path("link", shared.id).await.unwrap();
    let written = b
        .create_file("from_b.md", &shared.id, FileType::Document)
        .await
        .unwrap();
    b.write_document(written.id, b"written by b into a's folder")
        .await
        .unwrap();
    let mine = b.create_at_path("mine.md").await.unwrap();
    b.write_document(mine.id, b"b keeps this one")
        .await
        .unwrap();
    b.sync().await.unwrap();

    a.sync().await.unwrap();
    c.sync().await.unwrap();

    assert_synced_matches_server(&a).await;
    assert_synced_matches_server(&b).await;
    assert_synced_matches_server(&c).await;
}

#[tokio::test]
async fn moving_into_someone_elses_folder_frees_space() {
    let a = test_core_with_account().await;
    let b = test_core_with_account().await;
    let b_name = b.get_account().unwrap().username.clone();

    let shared = a.create_at_path("shared/").await.unwrap();
    a.share_file(shared.id, &b_name, ShareMode::Write)
        .await
        .unwrap();
    a.sync().await.unwrap();

    b.sync().await.unwrap();
    b.create_link_at_path("link", shared.id).await.unwrap();
    let doc = b.create_at_path("doc.md").await.unwrap();
    b.write_document(doc.id, b"moving out").await.unwrap();
    b.sync().await.unwrap();
    let before = b.get_usage().await.unwrap();

    b.move_file(&doc.id, &shared.id).await.unwrap();
    let moved = b.get_usage().await.unwrap();
    assert_eq!(moved.synced, before.synced);
    assert!(moved.freed.exact > METADATA_FEE);
    assert_eq!(moved.added.exact, 0);
    assert_eq!(moved.local.exact, before.synced.exact - moved.freed.exact);

    b.sync().await.unwrap();
    a.sync().await.unwrap();
    assert_eq!(b.get_usage().await.unwrap().synced, moved.local);
    assert_synced_matches_server(&a).await;
    assert_synced_matches_server(&b).await;
}

#[tokio::test]
async fn status_follows_local_changes() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("doc.md").await.unwrap();
    core.write_document(doc.id, b"unsynced").await.unwrap();
    let expected = core.get_usage().await.unwrap();
    assert!(expected.added.exact > 0);

    let mut status = core.status().await;
    for _ in 0..50 {
        if status.usage.local == expected.local {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        status = core.status().await;
    }
    assert_eq!(status.usage.local, expected.local);
    assert_eq!(status.usage.added, expected.added);
}
