use lb_rs::io::network::ApiError;
use lb_rs::model::api::*;
use lb_rs::model::crypto::AESEncrypted;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn get_document() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").await.unwrap().id;
    core.sync(None).await.unwrap();
    let old = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get()
        .get(&id)
        .unwrap()
        .clone();
    let mut new = old.clone();
    new.timestamped_value.value.set_hmac(Some([0; 32]));

    // update document content
    core.client
        .request(
            account,
            ChangeDocRequestV2 {
                diff: FileDiff::edit(old, new.clone()),
                new_content: AESEncrypted {
                    value: vec![69],
                    nonce: vec![69],
                    _t: Default::default(),
                },
            },
        )
        .await
        .unwrap();

    // get document
    let result = core
        .client
        .request(account, GetDocRequest { id, hmac: *new.document_hmac().unwrap() })
        .await
        .unwrap();
    assert_eq!(
        result.content,
        AESEncrypted { value: vec!(69), nonce: vec!(69), _t: Default::default() }
    );
}

#[tokio::test]
async fn get_document_not_found() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").await.unwrap().id;
    core.sync(None).await.unwrap();
    let mut old = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get()
        .get(&id)
        .unwrap()
        .clone();
    old.timestamped_value.value.set_id(Uuid::new_v4());
    let mut new = old;
    new.timestamped_value.value.set_hmac(Some([0; 32]));

    // get document we never created
    let result = core
        .client
        .request(account, GetDocRequest { id: *new.id(), hmac: *new.document_hmac().unwrap() })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::DocumentNotFound))
    );
}
