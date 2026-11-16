//! A share you haven't accepted yet should behave like one you have: browsable, readable offline,
//! exportable. Each test checks a pending share against what the same call does once accepted.

use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::FileType;
use lb_rs::service::import_export::ExportFileInfo;
use std::time::Duration;
use test_utils::*;

/// Owner shares `folder/` holding `doc.md`; the sharee syncs. Returns (owner, sharee, folder, doc).
async fn shared_folder(sharee: Lb) -> (Lb, Lb, uuid::Uuid, uuid::Uuid) {
    let owner = test_core_with_account().await;
    let folder = owner.create_at_path("folder/").await.unwrap();
    let doc = owner.create_at_path("folder/doc.md").await.unwrap();
    owner.write_document(doc.id, b"hello").await.unwrap();
    let sharee_name = sharee.get_account().unwrap().username.clone();
    owner
        .share_file(folder.id, &sharee_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    sharee.sync().await.unwrap();
    (owner, sharee, folder.id, doc.id)
}

#[tokio::test]
async fn pending_folder_lists_its_children() {
    let (_owner, sharee, folder, doc) = shared_folder(test_core_with_account().await).await;
    assert_eq!(sharee.get_pending_shares().await.unwrap()[0].id, folder);

    let children = sharee.get_children(&folder).await.unwrap();
    assert_eq!(children.into_iter().map(|f| f.id).collect::<Vec<_>>(), vec![doc]);
}

#[tokio::test]
async fn pending_folder_exports_its_documents() {
    let (_owner, sharee, folder, _doc) = shared_folder(test_core_with_account().await).await;
    let dest = tempfile::tempdir().unwrap();

    let progress = std::sync::Mutex::new(Vec::new());
    sharee
        .export_file(
            folder,
            dest.path().to_path_buf(),
            false,
            &Some(|i: ExportFileInfo| progress.lock().unwrap().push(i.lockbook_path)),
        )
        .await
        .unwrap();
    assert_eq!(progress.into_inner().unwrap(), vec![None, None]);

    let exported = std::fs::read(dest.path().join("folder").join("doc.md"));
    assert_eq!(exported.ok().as_deref(), Some(&b"hello"[..]));
}

#[tokio::test]
async fn pending_share_is_prefetched_like_an_accepted_one() {
    let sharee = test_core().await;
    let sharee = Lb::in_memory(
        lb_rs::model::core_config::Config { background_work: true, ..test_config() },
        sharee.client.clone(),
    )
    .await
    .unwrap();
    sharee
        .create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    let (owner, sharee, _folder, pending_doc) = shared_folder(sharee).await;

    // a second share, accepted, as the control
    let accepted = owner.create_at_path("accepted.md").await.unwrap();
    owner.write_document(accepted.id, b"hi").await.unwrap();
    let sharee_name = sharee.get_account().unwrap().username.clone();
    owner
        .share_file(accepted.id, &sharee_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    sharee.sync().await.unwrap();
    let root = sharee.root().await.unwrap();
    let link = sharee
        .create_file("accepted.md", &root.id, FileType::Link { target: accepted.id })
        .await
        .unwrap();
    assert_ne!(link.id, accepted.id);
    sharee.sync().await.unwrap();

    let (accepted_hmac, _) = owner
        .read_document_with_hmac(accepted.id, false)
        .await
        .unwrap();
    let (pending_hmac, _) = owner
        .read_document_with_hmac(pending_doc, false)
        .await
        .unwrap();

    // the post-sync fetcher runs in the background; wait for it to fetch the accepted share
    let mut waited = Duration::ZERO;
    while !sharee.docs.exists(accepted.id, accepted_hmac) {
        assert!(waited < Duration::from_secs(10), "accepted share was never prefetched");
        tokio::time::sleep(Duration::from_millis(50)).await;
        waited += Duration::from_millis(50);
    }

    assert!(
        sharee.docs.exists(pending_doc, pending_hmac),
        "pending share's document was not prefetched"
    );
}

#[tokio::test]
async fn a_pin_that_succeeds_is_listed() {
    let (_owner, sharee, _folder, doc) = shared_folder(test_core_with_account().await).await;

    if sharee.pin_file(doc).await.is_ok() {
        assert_eq!(sharee.list_pinned().await.unwrap(), vec![doc]);
    }
}

#[tokio::test]
async fn pending_files_have_no_path() {
    let (_owner, sharee, folder, doc) = shared_folder(test_core_with_account().await).await;

    assert!(sharee.get_path_by_id(doc).await.is_err());
    let paths = sharee.list_paths_with_ids(None).await.unwrap();
    assert!(!paths.iter().any(|(id, _)| *id == folder || *id == doc));
}
