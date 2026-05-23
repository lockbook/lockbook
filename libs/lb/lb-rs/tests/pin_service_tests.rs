use lb_rs::Lb;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_metadata::FileType;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn list_pinned_empty() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let pinned = core.list_pinned().await.unwrap();
    let expected: Vec<Uuid> = vec![];
    assert_eq!(pinned, expected);
}

#[tokio::test]
async fn pin_unpin_roundtrip() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let doc = core.create_at_path("hello.md").await.unwrap();
    core.pin_file(doc.id).await.unwrap();

    let pinned = core.list_pinned().await.unwrap();
    assert_eq!(pinned, vec![doc.id]);

    core.unpin_file(doc.id).await.unwrap();
    let pinned = core.list_pinned().await.unwrap();
    let expected: Vec<Uuid> = vec![];
    assert_eq!(pinned, expected);
}

#[tokio::test]
async fn pin_is_idempotent() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let doc = core.create_at_path("hello.md").await.unwrap();
    core.pin_file(doc.id).await.unwrap();
    core.pin_file(doc.id).await.unwrap();
    core.pin_file(doc.id).await.unwrap();

    let pinned = core.list_pinned().await.unwrap();
    assert_eq!(pinned, vec![doc.id]);
}

#[tokio::test]
async fn unpin_not_pinned_is_noop() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let doc = core.create_at_path("hello.md").await.unwrap();
    core.unpin_file(doc.id).await.unwrap();

    let pinned = core.list_pinned().await.unwrap();
    let expected: Vec<Uuid> = vec![];
    assert_eq!(pinned, expected);
}

#[tokio::test]
async fn pin_preserves_order() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let a = core.create_at_path("a.md").await.unwrap();
    let b = core.create_at_path("b.md").await.unwrap();
    let c = core.create_at_path("c.md").await.unwrap();

    core.pin_file(b.id).await.unwrap();
    core.pin_file(a.id).await.unwrap();
    core.pin_file(c.id).await.unwrap();

    let pinned = core.list_pinned().await.unwrap();
    assert_eq!(pinned, vec![b.id, a.id, c.id]);
}

#[tokio::test]
async fn pin_rejects_folder() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let root = core.root().await.unwrap();
    let folder = core
        .create_file("subdir", &root.id, FileType::Folder)
        .await
        .unwrap();

    let res = core.pin_file(folder.id).await;
    match res {
        Err(e) => assert!(matches!(e.kind, LbErrKind::FileNotDocument)),
        Ok(()) => panic!("expected pin_file to reject a folder"),
    }
}

#[tokio::test]
async fn pin_rejects_nonexistent() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let res = core.pin_file(Uuid::new_v4()).await;
    match res {
        Err(e) => assert!(matches!(e.kind, LbErrKind::FileNonexistent)),
        Ok(()) => panic!("expected pin_file to reject a missing id"),
    }
}

#[tokio::test]
async fn list_pinned_filters_deleted() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let keep = core.create_at_path("keep.md").await.unwrap();
    let gone = core.create_at_path("gone.md").await.unwrap();

    core.pin_file(keep.id).await.unwrap();
    core.pin_file(gone.id).await.unwrap();

    core.delete(&gone.id).await.unwrap();

    let pinned = core.list_pinned().await.unwrap();
    assert_eq!(pinned, vec![keep.id]);
}
