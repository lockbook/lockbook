#![cfg(not(target_family = "wasm"))]

use lb_rs::Lb;
use lb_rs::model::errors::LbErrKind;
use test_utils::test_core_with_account;

#[tokio::test]
async fn reads_catch_up_when_another_instance_replaces_the_document() {
    let writer = test_core_with_account().await;
    let id = writer.create_at_path("doc.md").await.unwrap().id;
    writer.write_document(id, b"old").await.unwrap();
    let (old_hmac, _) = writer.read_document_with_hmac(id, false).await.unwrap();
    let reader = Lb::init(writer.config.clone()).await.unwrap();
    let reader_with_hmac = Lb::init(writer.config.clone()).await.unwrap();

    writer.write_document(id, b"new").await.unwrap();
    let expected = writer.read_document_with_hmac(id, false).await.unwrap();
    assert_eq!(reader.read_document(id, false).await.unwrap(), b"old");
    writer.docs.delete(id, old_hmac).await.unwrap();

    assert_eq!(reader.read_document(id, false).await.unwrap(), b"new");
    assert_eq!(
        reader_with_hmac
            .read_document_with_hmac(id, false)
            .await
            .unwrap(),
        expected
    );
}

#[tokio::test]
async fn missing_version_catches_up_to_document_deletion() {
    let writer = test_core_with_account().await;
    let id = writer.create_at_path("doc.md").await.unwrap().id;
    writer.write_document(id, b"old").await.unwrap();
    let (hmac, _) = writer.read_document_with_hmac(id, false).await.unwrap();
    let reader = Lb::init(writer.config.clone()).await.unwrap();

    writer.delete(&id).await.unwrap();
    writer.docs.delete(id, hmac).await.unwrap();

    assert_eq!(reader.read_document(id, false).await.unwrap_err().kind, LbErrKind::FileNonexistent);
}

#[tokio::test]
async fn missing_version_is_fetched_from_the_server_after_catch_up() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("doc.md").await.unwrap().id;
    core.write_document(id, b"content").await.unwrap();
    core.sync().await.unwrap();
    let (hmac, _) = core.read_document_with_hmac(id, false).await.unwrap();
    core.docs.delete(id, hmac).await.unwrap();

    assert_eq!(core.read_document(id, false).await.unwrap(), b"content");
}
