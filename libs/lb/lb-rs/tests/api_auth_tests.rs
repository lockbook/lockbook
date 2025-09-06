use lb_rs::io::network::ApiError;
use lb_rs::model::api::*;
use lb_rs::model::crypto::AESEncrypted;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::*;

#[tokio::test]
async fn upsert_id_takeover() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_with_account().await;

    let acc1 = &core1.get_account().unwrap();
    let acc2 = &core2.get_account().unwrap();

    let mut file1 = {
        let id = core1.create_at_path("/test.md").await.unwrap().id;
        core1.sync(None).await.unwrap();

        core1
            .client
            .request(acc1, GetUpdatesRequest { since_metadata_version: 0 })
            .await
            .unwrap()
            .file_metadata
            .iter()
            .find(|&f| f.id() == &id)
            .unwrap()
            .clone()
    };

    file1.timestamped_value.value.parent = core2.root().await.unwrap().id;

    // If this succeeded account2 would be able to control file1
    let result = core2
        .client
        .request(acc2, UpsertRequest { updates: vec![FileDiff::new(file1)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldVersionRequired))
    );
}

#[tokio::test]
async fn upsert_id_takeover_change_parent() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_with_account().await;
    let account1 = core1.get_account().unwrap();
    let account2 = core2.get_account().unwrap();

    let file1 = {
        let id = core1.create_at_path("/test.md").await.unwrap().id;
        core1.sync(None).await.unwrap();
        core1
            .client
            .request(account1, GetUpdatesRequest { since_metadata_version: 0 })
            .await
            .unwrap()
            .file_metadata
            .iter()
            .find(|&f| f.id() == &id)
            .unwrap()
            .clone()
    };

    // If this succeeded account2 would be able to control file1
    let result = core2
        .client
        .request(account2, UpsertRequest { updates: vec![FileDiff::new(file1)] })
        .await;
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldVersionRequired))
    );
}

#[tokio::test]
async fn change_document_content() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_with_account().await;

    let file1 = {
        let id = core1.create_at_path("/test.md").await.unwrap().id;
        core1.sync(None).await.unwrap();

        let mut tx = core1.begin_tx().await;
        tx.db().base_metadata.get().get(&id).unwrap().clone()
    };

    let mut file2 = file1.clone();
    file2
        .timestamped_value
        .value
        .set_hmac_and_size(Some([0; 32]), Some(1));

    let acc2 = &core2.get_account().unwrap();
    let result = core2
        .client
        .request(
            acc2,
            ChangeDocRequestV2 {
                diff: FileDiff::edit(file1, file2),
                new_content: AESEncrypted {
                    value: vec![69],
                    nonce: vec![69],
                    _t: Default::default(),
                },
            },
        )
        .await;
    assert_matches!(
        result,
        Err(ApiError::<ChangeDocError>::Endpoint(ChangeDocError::NotPermissioned))
    );
}

#[tokio::test]
async fn get_someone_else_document() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_with_account().await;

    let file = {
        let id = core1.create_at_path("/test.md").await.unwrap().id;
        core1.write_document(id, &[1, 2, 3]).await.unwrap();
        core1.sync(None).await.unwrap();

        let mut tx = core1.begin_tx().await;
        tx.db().base_metadata.get().get(&id).unwrap().clone()
    };

    let req = GetDocRequest { id: *file.id(), hmac: *file.document_hmac().unwrap() };

    let acc2 = &core2.get_account().unwrap();
    let result = core2.client.request(acc2, req).await;
    assert_matches!(
        result,
        Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::NotPermissioned))
    );
}
