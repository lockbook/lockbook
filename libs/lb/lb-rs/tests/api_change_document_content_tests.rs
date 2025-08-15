use lb_rs::io::network::ApiError;
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
        .get()
        .get(&doc)
        .unwrap()
        .clone();

    // create document
    core.client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::new(doc.clone())] })
        .await
        .unwrap();

    let doc1 = doc;
    let mut doc2 = doc1.clone();
    doc2.timestamped_value
        .value
        .set_hmac_and_size(Some([0; 32]), Some(0));

    let diff = FileDiff::edit(doc1, doc2);
    // change document content
    core.client
        .request(
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
        .get()
        .get(&doc)
        .unwrap()
        .clone();

    // create document
    core.client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::new(doc.clone())] })
        .await
        .unwrap();

    doc.timestamped_value.value.set_id(Uuid::new_v4());
    let doc1 = doc;
    let mut doc2 = doc1.clone();
    doc2.timestamped_value
        .value
        .set_hmac_and_size(Some([0; 32]), Some(0));

    let diff = FileDiff::edit(doc1, doc2);
    // change document content
    let res = core
        .client
        .request(
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
