use db_rs::View;
use lb_rs::Lb;
use test_utils::test_config;
use uuid::Uuid;

#[tokio::test]
async fn independent_instances_open_the_same_database() {
    let config = test_config();
    let first = Lb::init(config.clone()).await.unwrap();
    let mut tx = first.begin_tx().await;
    tx.db().last_synced.replace(123).unwrap();
    tx.end();
    let second = Lb::init(config).await.unwrap();
    assert_eq!(second.ro_tx().await.db().last_synced.as_ref(), Some(&123));
}

#[tokio::test]
async fn write_transaction_catches_up_before_appending() {
    let config = test_config();
    let first = Lb::init(config.clone()).await.unwrap();
    let second = Lb::init(config).await.unwrap();
    let id = Uuid::new_v4();
    {
        let mut tx = first.begin_tx().await;
        tx.db().pinned_files.push(id).unwrap();
    }
    {
        let mut tx = second.begin_tx().await;
        assert_eq!(tx.db().pinned_files.as_slice(), [id]);
        tx.db().last_synced.replace(123).unwrap();
        tx.end();
    }
    let mut tx = first.begin_tx().await;
    assert_eq!(tx.db().last_synced.as_ref(), Some(&123));
    assert_eq!(tx.db().pinned_files.as_slice(), [id]);
    tx.end();
}

#[tokio::test]
async fn reads_remain_stale_until_an_empty_write_transaction() {
    let config = test_config();
    let first = Lb::init(config.clone()).await.unwrap();
    let second = Lb::init(config).await.unwrap();
    {
        let mut tx = first.begin_tx().await;
        tx.db().last_synced.replace(123).unwrap();
        tx.end();
    }
    assert!(second.ro_tx().await.db().last_synced.is_none());
    second.begin_tx().await.end();
    assert_eq!(second.ro_tx().await.db().last_synced.as_ref(), Some(&123));
}

#[tokio::test]
async fn catch_up_across_snapshots_keeps_deletions() {
    let config = test_config();
    let first = Lb::init(config.clone()).await.unwrap();
    let second = Lb::init(config).await.unwrap();
    let id = Uuid::new_v4();
    {
        let mut tx = first.begin_tx().await;
        tx.db().pinned_files.push(id).unwrap();
        tx.end();
    }
    second.begin_tx().await.end();
    first.db.write().await.snapshot().unwrap();
    {
        let mut tx = first.begin_tx().await;
        tx.db().pinned_files.clear().unwrap();
        tx.db().last_synced.replace(456).unwrap();
        tx.end();
    }
    first.db.write().await.snapshot().unwrap();
    second.begin_tx().await.end();
    let read = second.ro_tx().await;
    assert!(read.db().pinned_files.is_empty());
    assert_eq!(read.db().last_synced.as_ref(), Some(&456));
}
