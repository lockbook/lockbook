use lb_rs::io::network::ApiError;
use lb_rs::io::server::Server;
use lb_rs::model::api::*;
use lb_rs::model::crypto::AESEncrypted;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn get_document() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").await.unwrap().id;
    let created = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get(&id)
        .unwrap()
        .clone();
    let old = as_legacy(&created, None, None, account);
    Server::upsert_file_metadata_v2(
        &core.client,
        account,
        UpsertRequestV2 { updates: vec![FileDiff::new(old.clone())] },
    )
    .await
    .unwrap();
    let new = as_legacy(&created, Some([0; 32]), Some(1), account);

    // update document content
    Server::change_doc_v2(
        &core.client,
        account,
        ChangeDocRequestV2 {
            diff: FileDiff::edit(old, new),
            new_content: AESEncrypted { value: vec![69], nonce: vec![69], _t: Default::default() },
        },
    )
    .await
    .unwrap();

    // get document
    let result = Server::get_document(&core.client, account, GetDocRequest { id, hmac: [0; 32] })
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

    // get document we never created
    let result = Server::get_document(
        &core.client,
        account,
        GetDocRequest { id: Uuid::new_v4(), hmac: [0; 32] },
    )
    .await;
    assert_matches!(
        result,
        Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::DocumentNotFound))
    );
}
