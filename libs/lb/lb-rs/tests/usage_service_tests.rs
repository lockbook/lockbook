use lb_rs::service::usage::UsageItemMetric;
use test_utils::*;

#[tokio::test]
async fn get_uncompressed_usage_no_documents() {
    let core = test_core_with_account().await;

    assert_eq!(
        core.get_uncompressed_usage().await.unwrap(),
        UsageItemMetric { exact: 0, readable: "0 B".to_string() }
    );
}

#[tokio::test]
async fn get_uncompressed_usage_empty_document() {
    let core = test_core_with_account().await;
    let document = core.create_at_path("document").await.unwrap();
    core.write_document(document.id, b"").await.unwrap();
    assert_eq!(
        core.get_uncompressed_usage().await.unwrap(),
        UsageItemMetric { exact: 0, readable: "0 B".to_string() }
    );
}

#[tokio::test]
async fn get_uncompressed_usage_one_document() {
    let core = test_core_with_account().await;
    let document = core.create_at_path("document").await.unwrap();

    core.write_document(document.id, b"0123456789")
        .await
        .unwrap();

    assert_eq!(
        core.get_uncompressed_usage().await.unwrap(),
        UsageItemMetric { exact: 10, readable: "10 B".to_string() }
    );
}

#[tokio::test]
async fn get_uncompressed_usage_multiple_documents() {
    let core = test_core_with_account().await;
    let document1 = core.create_at_path("document1").await.unwrap();
    let document2 = core.create_at_path("document2").await.unwrap();

    core.write_document(document1.id, b"01234").await.unwrap();
    core.write_document(document2.id, b"56789").await.unwrap();

    assert_eq!(
        core.get_uncompressed_usage().await.unwrap(),
        UsageItemMetric { exact: 10, readable: "10 B".to_string() }
    );
}

#[tokio::test]
async fn get_uncompressed_usage_with_delete() {
    let core = test_core_with_account().await;
    let document1 = core.create_at_path("document1").await.unwrap();
    let document2 = core.create_at_path("document2").await.unwrap();

    core.write_document(document1.id, b"01234").await.unwrap();
    core.write_document(document2.id, b"56789").await.unwrap();

    core.delete(&document2.id).await.unwrap();

    assert_eq!(
        core.get_uncompressed_usage().await.unwrap(),
        UsageItemMetric { exact: 5, readable: "5 B".to_string() }
    );
}

#[tokio::test]
async fn get_uncompressed_usage_with_sync() {
    let core = test_core_with_account().await;
    let document1 = core.create_at_path("document1").await.unwrap();
    let document2 = core.create_at_path("document2").await.unwrap();

    core.write_document(document1.id, b"01234").await.unwrap();
    core.write_document(document2.id, b"56789").await.unwrap();
    core.sync(None).await.unwrap();
    let core2 = test_core_from(&core).await;

    assert_eq!(
        core2.get_uncompressed_usage().await.unwrap(),
        UsageItemMetric { exact: 10, readable: "10 B".to_string() }
    );
}
