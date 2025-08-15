use lb_rs::io::network::ApiError;
use lb_rs::model::ValidationFailure;
use lb_rs::model::api::*;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn move_document() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("doc.md").await.unwrap().id;
    let folder = core.create_at_path("folder/").await.unwrap().id;
    core.sync(None).await.unwrap();

    let mut tx = core.begin_tx().await;
    let doc1 = tx.db().base_metadata.get().get(&doc).unwrap().clone();

    // move document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.set_parent(folder);
    core.client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(doc1, doc2)] })
        .await
        .unwrap();
}

#[tokio::test]
async fn move_document_parent_not_found() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // create document and folder, but don't send folder to server
    let doc = core.create_at_path("folder/doc.md").await.unwrap().id;
    core.sync(None).await.unwrap();

    let mut tx = core.begin_tx().await;
    let doc1 = tx.db().base_metadata.get().get(&doc).unwrap().clone();

    // move document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.set_parent(Uuid::new_v4());

    let result = core
        .client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(doc1, doc2)] })
        .await;
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}

#[tokio::test]
async fn move_document_deleted() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let doc = core.create_at_path("doc.md").await.unwrap().id;
    let folder = core.create_at_path("folder/").await.unwrap().id;

    let mut tx = core.begin_tx().await;
    let doc1 = tx.db().local_metadata.get().get(&doc).unwrap().clone();
    let folder = tx.db().local_metadata.get().get(&folder).unwrap().clone();

    core.client
        .request(
            account,
            UpsertRequestV2 {
                updates: vec![FileDiff::new(doc1.clone()), FileDiff::new(folder.clone())],
            },
        )
        .await
        .unwrap();

    // move & delete document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.set_deleted(true);
    doc2.timestamped_value.value.set_parent(*folder.id());
    let result = core
        .client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(doc1, doc2)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(
            ValidationFailure::DeletedFileUpdated(_)
        )))
    );
}

#[tokio::test]
async fn move_document_path_taken() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let root = core.root().await.unwrap();

    let folder = core.create_at_path("folder/").await.unwrap().id;
    let folder = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&folder)
        .unwrap()
        .clone();

    let doc = core.create_at_path("doc.md").await.unwrap().id;
    let doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&doc)
        .unwrap()
        .clone();

    let doc2 = core.create_at_path("folder/doc.md").await.unwrap().id;
    let doc2 = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&doc2)
        .unwrap()
        .clone();

    core.client
        .request(
            account,
            UpsertRequestV2 {
                updates: vec![
                    FileDiff::new(doc.clone()),
                    FileDiff::new(doc2.clone()),
                    FileDiff::new(folder),
                ],
            },
        )
        .await
        .unwrap();

    let mut new = doc2.clone();
    new.timestamped_value.value.set_parent(root.id);
    new.timestamped_value
        .value
        .set_name(doc.secret_name().clone());

    let result = core
        .client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(doc2, new)] })
        .await;

    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}

#[tokio::test]
async fn move_folder_into_itself() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let folder = core.create_at_path("folder/").await.unwrap().id;
    let folder = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&folder)
        .unwrap()
        .clone();

    core.client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::new(folder.clone())] })
        .await
        .unwrap();

    let mut new = folder.clone();
    new.timestamped_value.value.set_parent(*new.id());

    let result = core
        .client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(folder, new)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(ValidationFailure::Cycle(
            _
        ))))
    );
}

#[tokio::test]
async fn move_folder_into_descendants() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let folder = core.create_at_path("folder1/").await.unwrap().id;
    let folder = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&folder)
        .unwrap()
        .clone();

    let folder2 = core.create_at_path("folder1/folder2/").await.unwrap().id;
    let folder2 = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&folder2)
        .unwrap()
        .clone();

    core.client
        .request(
            account,
            UpsertRequestV2 {
                updates: vec![FileDiff::new(folder.clone()), FileDiff::new(folder2.clone())],
            },
        )
        .await
        .unwrap();

    let mut folder_new = folder.clone();
    folder_new.timestamped_value.value.set_parent(*folder2.id());
    let result = core
        .client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(folder, folder_new)] })
        .await;
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}

#[tokio::test]
async fn move_document_into_document() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // create documents
    let doc = core.create_at_path("doc1.md").await.unwrap().id;
    let doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&doc)
        .unwrap()
        .clone();

    let doc2 = core.create_at_path("doc2.md").await.unwrap().id;
    let doc2 = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&doc2)
        .unwrap()
        .clone();

    core.client
        .request(
            account,
            UpsertRequestV2 {
                updates: vec![FileDiff::new(doc.clone()), FileDiff::new(doc2.clone())],
            },
        )
        .await
        .unwrap();

    // move folder into itself
    let mut new = doc.clone();
    new.timestamped_value.value.set_parent(*doc2.id());
    let result = core
        .client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(doc, new)] })
        .await;
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}
