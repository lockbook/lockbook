use lb_rs::io::network::ApiError;
use lb_rs::model::api::*;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn delete_document() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").await.unwrap().id;
    core.sync(None).await.unwrap();

    let doc1 = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get()
        .get(&doc)
        .unwrap()
        .clone();
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.is_deleted = true;
    core.client
        .request(account, UpsertRequest { updates: vec![FileDiff::edit(doc1, doc2)] })
        .await
        .unwrap();
}

#[tokio::test]
async fn delete_document_not_found() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").await.unwrap().id;
    core.sync(None).await.unwrap();
    let mut doc1 = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get()
        .get(&doc)
        .unwrap()
        .clone();
    doc1.timestamped_value.value.id = Uuid::new_v4();

    // delete document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.is_deleted = true;
    let result = core
        .client
        .request(
            account,
            UpsertRequest {
                // create document as if deleting an existing document
                updates: vec![FileDiff::edit(doc1, doc2)],
            },
        )
        .await;
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldFileNotFound)));
}

#[tokio::test]
async fn delete_document_new_document() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let doc = core.create_at_path("test.md").await.unwrap().id;
    let mut doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&doc)
        .unwrap()
        .clone();
    doc.timestamped_value.value.is_deleted = true;

    let result = core
        .client
        .request(account, UpsertRequest { updates: vec![FileDiff::new(doc)] })
        .await;
    assert_matches!(result, Ok(_));
}

#[tokio::test]
async fn delete_document_deleted() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let doc = core.create_at_path("test.md").await.unwrap().id;
    let doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&doc)
        .unwrap()
        .clone();
    core.sync(None).await.unwrap();

    // delete document
    let mut doc2 = doc.clone();
    doc2.timestamped_value.value.is_deleted = true;
    core.client
        .request(account, UpsertRequest { updates: vec![FileDiff::edit(doc, doc2)] })
        .await
        .unwrap();
}

#[tokio::test]
async fn delete_cannot_delete_root() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let root = core.root().await.unwrap().id;
    let root1 = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get()
        .get(&root)
        .unwrap()
        .clone();

    let mut root2 = root1.clone();
    root2.timestamped_value.value.is_deleted = true;
    let result = core
        .client
        .request(account, UpsertRequest { updates: vec![FileDiff::edit(root1, root2)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::RootModificationInvalid))
    );
}
