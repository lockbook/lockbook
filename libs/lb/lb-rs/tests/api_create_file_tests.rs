use lb_rs::io::network::ApiError;
use lb_rs::model::ValidationFailure;
use lb_rs::model::api::*;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn create_document() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").await.unwrap().id;
    let doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&id)
        .unwrap()
        .clone();

    core.client
        .request(account, UpsertRequest { updates: vec![FileDiff::new(doc)] })
        .await
        .unwrap();
}

#[tokio::test]
async fn create_document_duplicate_id() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").await.unwrap().id;
    let doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&id)
        .unwrap()
        .clone();

    core.sync(None).await.unwrap();

    // create document with same id and key
    let result = core
        .client
        .request(account, UpsertRequest { updates: vec![FileDiff::new(doc)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldVersionRequired))
    );
}

#[tokio::test]
async fn create_document_duplicate_path() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // create document
    let id = core.create_at_path("test.md").await.unwrap().id;
    let mut doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&id)
        .unwrap()
        .clone();
    core.sync(None).await.unwrap();

    // create document with same path
    doc.timestamped_value.value.id = Uuid::new_v4();
    let result = core
        .client
        .request(account, UpsertRequest { updates: vec![FileDiff::new(doc)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(
            ValidationFailure::PathConflict(_)
        )))
    );
}

#[tokio::test]
async fn create_document_parent_not_found() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // create document
    let id = core.create_at_path("parent/test.md").await.unwrap().id;
    let doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&id)
        .unwrap()
        .clone();

    let result = core
        .client
        .request(account, UpsertRequest { updates: vec![FileDiff::new(doc)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(ValidationFailure::Orphan(
            _
        ))))
    );
}
