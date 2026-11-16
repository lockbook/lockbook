use std::time::Duration;
use test_utils::*;
use uuid::Uuid;

async fn empty_query(core: &Lb) -> Vec<Uuid> {
    let searcher = core.path_searcher().await;
    searcher.results().iter().map(|result| result.id).collect()
}

#[tokio::test]
async fn accepted_results_lead_an_empty_query() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let mut ids = vec![];
    for path in ["a.md", "b.md", "c.md"] {
        ids.push(core.create_at_path(path).await.unwrap().id);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let [a, b, c] = ids[..] else { unreachable!() };
    assert_eq!(empty_query(&core).await, [c, b, a]);

    core.search_accepted(a).await.unwrap();
    assert_eq!(empty_query(&core).await, [a, c, b]);

    core.search_accepted(b).await.unwrap();
    core.search_accepted(a).await.unwrap();
    assert_eq!(empty_query(&core).await, [a, b, c]);
}

#[tokio::test]
async fn accepted_results_do_not_reorder_a_query() {
    let core: Lb = test_core().await;
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let apple = core.create_at_path("apple.md").await.unwrap().id;
    let pineapple = core.create_at_path("pineapple.md").await.unwrap().id;
    core.search_accepted(pineapple).await.unwrap();

    let mut searcher = core.path_searcher().await;
    searcher.query("apple");
    let ids: Vec<Uuid> = searcher.results().iter().map(|result| result.id).collect();
    assert_eq!(ids, [apple, pineapple]);
}
