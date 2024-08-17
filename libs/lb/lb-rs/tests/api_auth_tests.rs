use lb_rs::service::api_service::{ApiError, Requester};
use lb_rs::shared::api::*;
use lb_rs::shared::crypto::AESEncrypted;
use lb_rs::shared::file_like::FileLike;
use lb_rs::shared::file_metadata::FileDiff;
use test_utils::*;

#[test]
fn upsert_id_takeover() {
    let core1 = test_core_with_account();
    let core2 = test_core_with_account();

    let acc1 = &core1.get_account().unwrap();
    let acc2 = &core2.get_account().unwrap();

    let mut file1 = {
        let id = core1.create_at_path("/test.md").unwrap().id;
        core1.sync(None).unwrap();
        core1
            .in_tx(|s| {
                Ok(s.client
                    .request(acc1, GetUpdatesRequest { since_metadata_version: 0 })
                    .unwrap()
                    .file_metadata
                    .iter()
                    .find(|&f| f.id() == &id)
                    .unwrap()
                    .clone())
            })
            .unwrap()
    };

    file1.timestamped_value.value.parent = core2.get_root().unwrap().id;

    // If this succeeded account2 would be able to control file1
    core2
        .in_tx(|s| {
            let result = s
                .client
                .request(acc2, UpsertRequest { updates: vec![FileDiff::new(&file1)] });
            assert_matches!(
                result,
                Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldVersionRequired))
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn upsert_id_takeover_change_parent() {
    let core1 = test_core_with_account();
    let core2 = test_core_with_account();
    let account1 = core1.get_account().unwrap();
    let account2 = core2.get_account().unwrap();

    let file1 = {
        let id = core1.create_at_path("/test.md").unwrap().id;
        core1.sync(None).unwrap();
        core1
            .in_tx(|s| {
                Ok(s.client
                    .request(&account1, GetUpdatesRequest { since_metadata_version: 0 })
                    .unwrap()
                    .file_metadata
                    .iter()
                    .find(|&f| f.id() == &id)
                    .unwrap()
                    .clone())
            })
            .unwrap()
    };

    // If this succeeded account2 would be able to control file1
    core2
        .in_tx(|s| {
            let result = s
                .client
                .request(&account2, UpsertRequest { updates: vec![FileDiff::new(&file1)] });
            assert_matches!(
                result,
                Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldVersionRequired))
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn change_document_content() {
    let core1 = test_core_with_account();
    let core2 = test_core_with_account();

    let file1 = {
        let id = core1.create_at_path("/test.md").unwrap().id;
        core1.sync(None).unwrap();
        core1
            .in_tx(|s| Ok(s.db.base_metadata.get().get(&id).unwrap().clone()))
            .unwrap()
    };

    let mut file2 = file1.clone();
    file2.timestamped_value.value.document_hmac = Some([0; 32]);

    let acc2 = &core2.get_account().unwrap();
    core2
        .in_tx(|s| {
            let result = s.client.request(
                acc2,
                ChangeDocRequest {
                    diff: FileDiff::edit(&file1, &file2),
                    new_content: AESEncrypted {
                        value: vec![69],
                        nonce: vec![69],
                        _t: Default::default(),
                    },
                },
            );
            assert_matches!(
                result,
                Err(ApiError::<ChangeDocError>::Endpoint(ChangeDocError::NotPermissioned))
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn get_someone_else_document() {
    let core1 = test_core_with_account();
    let core2 = test_core_with_account();

    let file = {
        let id = core1.create_at_path("/test.md").unwrap().id;
        core1.write_document(id, &[1, 2, 3]).unwrap();
        core1.sync(None).unwrap();
        core1
            .in_tx(|s| Ok(s.db.base_metadata.get().get(&id).unwrap().clone()))
            .unwrap()
    };

    let req = GetDocRequest { id: *file.id(), hmac: *file.document_hmac().unwrap() };

    let acc2 = &core2.get_account().unwrap();
    core2
        .in_tx(|s| {
            let result = s.client.request(acc2, req);
            assert_matches!(
                result,
                Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::NotPermissioned))
            );
            Ok(())
        })
        .unwrap();
}
