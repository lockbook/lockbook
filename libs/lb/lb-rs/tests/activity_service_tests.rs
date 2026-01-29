use lb_rs::Lb;
use lb_rs::service::activity::RankingWeights;
use test_utils::*;
use tokio::time;
use uuid::Uuid;
use web_time::Duration;

#[tokio::test]
async fn suggest_docs() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document = core.create_at_path("hello.md").await.unwrap();
    core.write_document(document.id, "hello world".as_bytes())
        .await
        .unwrap();
    time::sleep(Duration::from_millis(100)).await;

    let expected_suggestions = core
        .suggested_docs(RankingWeights::default())
        .await
        .unwrap();

    assert_eq!(vec![document.id], expected_suggestions);
}

#[tokio::test]
async fn suggest_docs_empty() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let expected = core
        .suggested_docs(RankingWeights::default())
        .await
        .unwrap();
    let actual: Vec<Uuid> = vec![];

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn write_count() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document1 = core.create_at_path("hello1.md").await.unwrap();
    for _ in 0..10 {
        core.write_document(document1.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    let document2 = core.create_at_path("hello2.md").await.unwrap();
    for _ in 0..20 {
        core.write_document(document2.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    time::sleep(Duration::from_millis(100)).await;
    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 0, io: 100 })
        .await
        .unwrap();
    let expected_suggestions = vec![document2.id, document1.id];
    assert_eq!(actual_suggestions, expected_suggestions);
}

#[tokio::test]
async fn write_count_multiple_docs() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document1 = core.create_at_path("hello.md").await.unwrap();
    for _ in 0..10 {
        core.write_document(document1.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    let document2 = core.create_at_path("hello2.md").await.unwrap();
    for _ in 0..50 {
        core.write_document(document2.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    let document3 = core.create_at_path("hello3.md").await.unwrap();
    for _ in 0..55 {
        core.write_document(document3.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    time::sleep(Duration::from_millis(100)).await;
    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 0, io: 100 })
        .await
        .unwrap();

    let expected_suggestions = vec![document3.id, document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}

#[tokio::test]
async fn clear_docs() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document1 = core.create_at_path("hello.md").await.unwrap();
    for _ in 0..10 {
        core.write_document(document1.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    let document2 = core.create_at_path("hello2.md").await.unwrap();
    for _ in 0..50 {
        core.write_document(document2.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    let document3 = core.create_at_path("hello3.md").await.unwrap();
    for _ in 0..55 {
        core.write_document(document3.id, "hello world".as_bytes())
            .await
            .unwrap();
    }

    time::sleep(Duration::from_millis(100)).await;

    core.clear_suggested_id(document2.id).await.unwrap();

    let expected_suggestions = vec![document3.id, document1.id];
    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 0, io: 100 })
        .await
        .unwrap();
    assert_eq!(actual_suggestions, expected_suggestions);

    core.clear_suggested().await.unwrap();

    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 0, io: 100 })
        .await
        .unwrap();
    let expected_suggestions = vec![];

    assert_eq!(actual_suggestions, expected_suggestions);
}

#[tokio::test]
async fn read_count() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document1 = core.create_at_path("hello1.md").await.unwrap();
    for _ in 0..10 {
        core.read_document(document1.id, true).await.unwrap();
    }

    let document2 = core.create_at_path("hello2.md").await.unwrap();
    for _ in 0..20 {
        core.read_document(document2.id, true).await.unwrap();
    }

    time::sleep(Duration::from_millis(100)).await;
    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 0, io: 100 })
        .await
        .unwrap();
    let expected_suggestions = vec![document2.id, document1.id];
    assert_eq!(actual_suggestions, expected_suggestions);
}

#[tokio::test]
async fn read_count_multiple_docs() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document1 = core.create_at_path("hello.md").await.unwrap();
    for _ in 0..10 {
        core.read_document(document1.id, true).await.unwrap();
    }

    let document2 = core.create_at_path("hello2.md").await.unwrap();
    for _ in 0..20 {
        core.read_document(document2.id, true).await.unwrap();
    }

    let document3 = core.create_at_path("hello3.md").await.unwrap();
    for _ in 0..100 {
        core.read_document(document3.id, true).await.unwrap();
    }

    time::sleep(Duration::from_millis(100)).await;
    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 0, io: 100 })
        .await
        .unwrap();

    let expected_suggestions = vec![document3.id, document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}

#[tokio::test]
async fn last_read() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document1 = core.create_at_path("hello.md").await.unwrap();
    core.read_document(document1.id, true).await.unwrap();

    time::sleep(Duration::from_millis(100)).await;

    let document2 = core.create_at_path("hello2.md").await.unwrap();
    core.read_document(document2.id, true).await.unwrap();

    time::sleep(Duration::from_millis(100)).await;
    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 100, io: 0 })
        .await
        .unwrap();

    let expected_suggestions = vec![document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}

#[tokio::test]
async fn last_write() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let document1 = core.create_at_path("hello.md").await.unwrap();
    core.write_document(document1.id, "hello world".as_bytes())
        .await
        .unwrap();

    time::sleep(Duration::from_millis(100)).await;

    let document2 = core.create_at_path("hello2.md").await.unwrap();
    core.write_document(document2.id, "hello world".as_bytes())
        .await
        .unwrap();

    time::sleep(Duration::from_millis(100)).await;
    let actual_suggestions = core
        .suggested_docs(RankingWeights { temporality: 100, io: 0 })
        .await
        .unwrap();

    let expected_suggestions = vec![document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}
