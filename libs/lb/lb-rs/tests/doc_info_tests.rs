use lb_rs::io::server::Server;
use lb_rs::model::api::{
    ChangeDocRequestV2, GetDocRequestV3, GetUpdatesRequestV2, UpsertRequestV2,
};
use lb_rs::model::doc_info::VersionKind;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileDiff;
use lb_rs::model::signed_meta::SignedMeta;
use lb_rs::model::tree_like::TreeLike;
use lb_rs::service::documents::compress_encrypt_document;
use test_utils::*;

#[cfg(not(feature = "in-process-tests"))]
mod over_http {
    use lb_rs::io::network::{ApiError, Network};
    use lb_rs::io::server::Server;
    use lb_rs::model::api::{GetDocUpdatesRequest, GetUpdatesError, GetUpdatesRequestV2};
    use lb_rs::model::clock::{Timestamp, get_time};
    use test_utils::*;
    use uuid::Uuid;

    static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(get_time().0 - 3600000);
    static OLD_VERSION: fn() -> &'static str = || "26.10.30";

    async fn learn_remote_head(core: &Lb, id: Uuid) {
        let account = core.get_account().unwrap();
        let heads = Server::get_doc_updates(
            &core.client,
            account,
            GetDocUpdatesRequest { since: 0, ids: vec![id] },
        )
        .await
        .unwrap()
        .heads;
        let (_, head) = heads
            .into_iter()
            .find(|(head_id, _)| *head_id == id)
            .unwrap();
        let mut tx = core.begin_tx().await;
        let db = tx.db();
        let mut info = db.doc_info.get(&id).copied().unwrap_or_default();
        info.remote = Some(head);
        db.doc_info.insert(id, info).unwrap();
    }

    fn offline(core: &Lb) -> Lb {
        let mut offline = core.clone();
        offline.client = Network { get_time: EARLY_CLOCK, ..Default::default() };
        offline
    }

    #[tokio::test]
    async fn offline_read_and_write_use_the_local_base() {
        let c1 = test_core_with_account().await;
        let doc = c1.create_at_path("doc.md").await.unwrap();
        c1.write_document(doc.id, b"line a\nline b\n")
            .await
            .unwrap();
        c1.sync().await.unwrap();

        let c2 = test_core_from(&c1).await;
        assert_eq!(c2.read_document(doc.id, false).await.unwrap(), b"line a\nline b\n");

        c1.write_document(doc.id, b"line a\nline b remote\n")
            .await
            .unwrap();
        c1.sync().await.unwrap();
        learn_remote_head(&c2, doc.id).await;

        let c2_offline = offline(&c2);
        let (hmac, content) = c2_offline
            .read_document_with_hmac(doc.id, false)
            .await
            .unwrap();
        assert_eq!(content, b"line a\nline b\n");
        c2_offline
            .safe_write(doc.id, hmac, b"line a offline\nline b\n".to_vec(), None)
            .await
            .unwrap();
        assert!(c2_offline.sync().await.is_err());

        c2.sync().await.unwrap();
        c1.sync().await.unwrap();
        let merged = b"line a offline\nline b remote\n";
        assert_eq!(c1.read_document(doc.id, false).await.unwrap(), merged);
        assert_eq!(c2.read_document(doc.id, false).await.unwrap(), merged);
    }

    #[tokio::test]
    async fn old_clients_are_asked_to_update_once_v3_metadata_exists() {
        let core = test_core_with_account().await;
        let account = core.get_account().unwrap();

        let old = Network { get_code_version: OLD_VERSION, ..Default::default() };
        let result = old
            .request(account, GetUpdatesRequestV2 { since_metadata_version: 0 })
            .await;
        assert_matches!(result, Err(ApiError::<GetUpdatesError>::ClientUpdateRequired));

        Server::get_updates_v2(
            &core.client,
            account,
            GetUpdatesRequestV2 { since_metadata_version: 0 },
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn content_push_does_not_need_current_metadata() {
    let c1 = test_core_with_account().await;
    let doc = c1.create_at_path("doc.md").await.unwrap();
    c1.write_document(doc.id, b"first").await.unwrap();
    c1.sync().await.unwrap();

    let c2 = test_core_from(&c1).await;
    let folder = c2.create_at_path("folder/").await.unwrap();
    c2.rename_file(&doc.id, "renamed.md").await.unwrap();
    c2.move_file(&doc.id, &folder.id).await.unwrap();
    c2.sync().await.unwrap();

    c1.write_document(doc.id, b"second").await.unwrap();
    c1.sync_docs().await.unwrap();

    c2.sync_docs().await.unwrap();
    assert_eq!(c2.read_document(doc.id, false).await.unwrap(), b"second");

    c1.sync().await.unwrap();
    assert_eq!(c1.get_file_by_id(doc.id).await.unwrap().name, "renamed.md");
    assert_eq!(c1.read_document(doc.id, false).await.unwrap(), b"second");
    contents_match(&c1, &c2).await;
}

#[tokio::test]
async fn small_edits_travel_as_diffs_until_a_snapshot_is_required() {
    let c1 = test_core_with_account().await;
    let account = c1.get_account().unwrap();
    let doc = c1.create_at_path("doc.md").await.unwrap();
    let base: String = (0..400)
        .map(|i| format!("line {i} {}\n", i * 7919 % 104729))
        .collect();
    c1.write_document(doc.id, base.as_bytes()).await.unwrap();
    c1.sync().await.unwrap();

    let mut content = base.clone();
    for i in 0..3 {
        content.push_str(&format!("edit {i}\n"));
        c1.write_document(doc.id, content.as_bytes()).await.unwrap();
        c1.sync_docs().await.unwrap();
    }

    let versions = Server::get_doc_v3(
        &c1.client,
        account,
        GetDocRequestV3 { id: doc.id, have: None, want: None },
    )
    .await
    .unwrap()
    .versions;
    assert_eq!(versions.len(), 4);
    assert_eq!(versions[0].0.kind, VersionKind::Snapshot);
    assert!(
        versions[1..]
            .iter()
            .all(|(version, _)| matches!(version.kind, VersionKind::Diff { .. }))
    );

    let c2 = test_core_from(&c1).await;
    assert_eq!(c2.read_document(doc.id, false).await.unwrap(), content.as_bytes());

    for i in 0..200 {
        content = content.replacen(&format!("line {i} "), &format!("line {i}! "), 1);
        c1.write_document(doc.id, content.as_bytes()).await.unwrap();
        c1.sync_docs().await.unwrap();
    }
    let response = Server::get_doc_v3(
        &c1.client,
        account,
        GetDocRequestV3 { id: doc.id, have: None, want: None },
    )
    .await
    .unwrap();
    let snapshot = response.versions[0].0.compressed_size;
    let stored: u64 = response
        .versions
        .iter()
        .map(|(version, _)| version.compressed_size)
        .sum();
    assert!(stored <= 2 * snapshot);
    assert_eq!(response.head.unwrap().stored_size, stored);

    c2.sync().await.unwrap();
    assert_eq!(c2.read_document(doc.id, false).await.unwrap(), content.as_bytes());
}

#[tokio::test]
async fn conflicting_push_pulls_merges_and_retries() {
    let c1 = test_core_with_account().await;
    let doc = c1.create_at_path("doc.md").await.unwrap();
    c1.write_document(doc.id, b"top\nmiddle\nbottom\n")
        .await
        .unwrap();
    c1.sync().await.unwrap();

    let c2 = test_core_from(&c1).await;
    assert_eq!(c2.read_document(doc.id, false).await.unwrap(), b"top\nmiddle\nbottom\n");

    c1.write_document(doc.id, b"top\nmiddle\nbottom c1\n")
        .await
        .unwrap();
    c1.sync().await.unwrap();

    c2.write_document(doc.id, b"top c2\nmiddle\nbottom\n")
        .await
        .unwrap();
    c2.begin_tx()
        .await
        .db()
        .last_doc_synced
        .replace(u64::MAX)
        .unwrap();
    c2.sync_docs().await.unwrap();

    c1.sync().await.unwrap();
    let merged = b"top c2\nmiddle\nbottom c1\n";
    assert_eq!(c1.read_document(doc.id, false).await.unwrap(), merged);
    assert_eq!(c2.read_document(doc.id, false).await.unwrap(), merged);
}

#[tokio::test]
async fn read_fetches_content_that_was_never_pulled() {
    let c1 = test_core_with_account().await;
    let doc = c1.create_at_path("doc.bin").await.unwrap();
    c1.write_document(doc.id, b"binary-ish").await.unwrap();
    c1.sync().await.unwrap();

    let c2 = test_core_from(&c1).await;
    assert!(c2.document_hmac(doc.id).await.unwrap().is_some());
    let (hmac, content) = c2.read_document_with_hmac(doc.id, false).await.unwrap();
    assert_eq!(content, b"binary-ish");
    assert_eq!(hmac, c2.document_hmac(doc.id).await.unwrap());

    assert!(
        c2.safe_write(doc.id, None, b"blind".to_vec(), None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn legacy_content_reaches_new_clients_and_their_writes_upgrade_the_meta() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let id = core.create_at_path("legacy.md").await.unwrap().id;
    let (created, key) = {
        let tx = core.ro_tx().await;
        let db = tx.db();
        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let key = tree.decrypt_key(&id, &core.keychain).unwrap();
        (db.local_metadata.get(&id).unwrap().clone(), key)
    };

    let empty = as_legacy(&created, None, None, account);
    Server::upsert_file_metadata_v2(
        &core.client,
        account,
        UpsertRequestV2 { updates: vec![FileDiff::new(empty.clone())] },
    )
    .await
    .unwrap();
    let (hmac, content) = compress_encrypt_document(&key, b"from an old client").unwrap();
    let written = as_legacy(&created, Some(hmac), Some(content.value.len()), account);
    Server::change_doc_v2(
        &core.client,
        account,
        ChangeDocRequestV2 { diff: FileDiff::edit(empty, written), new_content: content },
    )
    .await
    .unwrap();

    let other = test_core_from(&core).await;
    assert_eq!(other.read_document(id, false).await.unwrap(), b"from an old client");

    let pulled =
        |updates: Vec<SignedMeta>| updates.into_iter().find(|meta| meta.id() == &id).unwrap();
    let before = pulled(
        Server::get_updates_v2(
            &other.client,
            account,
            GetUpdatesRequestV2 { since_metadata_version: 0 },
        )
        .await
        .unwrap()
        .file_metadata,
    );
    assert!(!before.timestamped_value.value.is_v3());

    other
        .write_document(id, b"from a new client")
        .await
        .unwrap();
    other.sync().await.unwrap();

    let after = pulled(
        Server::get_updates_v2(
            &other.client,
            account,
            GetUpdatesRequestV2 { since_metadata_version: 0 },
        )
        .await
        .unwrap()
        .file_metadata,
    );
    assert!(after.timestamped_value.value.is_v3());
    assert_eq!(after.parent(), before.parent());
}

async fn contents_match(left: &Lb, right: &Lb) {
    let left_files = left.list_metadatas().await.unwrap();
    for file in left_files.iter().filter(|file| file.is_document()) {
        assert_eq!(
            left.read_document(file.id, false).await.unwrap(),
            right.read_document(file.id, false).await.unwrap()
        );
    }
}
