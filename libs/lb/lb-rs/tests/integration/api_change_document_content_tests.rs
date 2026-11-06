use lb_rs::io::network::ApiError;
use lb_rs::io::server::Server;
use lb_rs::model::api::*;
use lb_rs::model::crypto::AESEncrypted;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::{assert_matches, *};
use uuid::Uuid;

#[tokio::test]
async fn change_document_content() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").await.unwrap().id;
    let doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get(&doc)
        .unwrap()
        .clone();

    let doc1 = as_legacy(&doc, None, None, account);

    // create document
    Server::upsert_file_metadata_v2(
        &core.client,
        account,
        UpsertRequestV2 { updates: vec![FileDiff::new(doc1.clone())] },
    )
    .await
    .unwrap();

    let doc2 = as_legacy(&doc, Some([0; 32]), Some(0), account);

    let diff = FileDiff::edit(doc1, doc2);
    // change document content
    Server::change_doc_v2(
        &core.client,
        account,
        ChangeDocRequestV2 {
            diff,
            new_content: AESEncrypted { value: vec![], nonce: vec![], _t: Default::default() },
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn change_document_content_not_found() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").await.unwrap().id;
    let mut doc = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get(&doc)
        .unwrap()
        .clone();

    // create document
    Server::upsert_file_metadata_v2(
        &core.client,
        account,
        UpsertRequestV2 { updates: vec![FileDiff::new(doc.clone())] },
    )
    .await
    .unwrap();

    doc.timestamped_value.value.set_id(Uuid::new_v4());
    let doc1 = as_legacy(&doc, None, None, account);
    let doc2 = as_legacy(&doc, Some([0; 32]), Some(0), account);

    let diff = FileDiff::edit(doc1, doc2);
    // change document content
    let res = Server::change_doc_v2(
        &core.client,
        account,
        ChangeDocRequestV2 {
            diff,
            new_content: AESEncrypted { value: vec![], nonce: vec![], _t: Default::default() },
        },
    )
    .await;
    assert_matches!(
        res,
        Err(ApiError::<ChangeDocError>::Endpoint(ChangeDocError::DocumentNotFound))
    );
}
