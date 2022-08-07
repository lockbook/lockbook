use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::*;
use lockbook_shared::crypto::AESEncrypted;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileDiff;
use test_utils::*;

#[test]
fn upsert_id_takeover() {
    let core1 = test_core_with_account();
    let core2 = test_core_with_account();

    let mut file1 = {
        let id = core1.create_at_path("/test.md").unwrap().id;
        core1.sync(None).unwrap();
        api_service::request(
            &core1.get_account().unwrap(),
            GetUpdatesRequest { since_metadata_version: 0 },
        )
        .unwrap()
        .file_metadata
        .iter()
        .find(|&f| f.id() == &id)
        .unwrap()
        .clone()
    };

    file1.timestamped_value.value.parent = core2.get_root().unwrap().id;

    // If this succeeded account2 would be able to control file1
    let result = api_service::request(
        &core2.get_account().unwrap(),
        UpsertRequest { updates: vec![FileDiff::new(&file1)] },
    );
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::NotPermissioned)));
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
        api_service::request(&account1, GetUpdatesRequest { since_metadata_version: 0 })
            .unwrap()
            .file_metadata
            .iter()
            .find(|&f| f.id() == &id)
            .unwrap()
            .clone()
    };

    // If this succeeded account2 would be able to control file1
    let result =
        api_service::request(&account2, UpsertRequest { updates: vec![FileDiff::new(&file1)] });
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::NotPermissioned)));
}

#[test]
fn change_document_content() {
    let core1 = test_core_with_account();
    let core2 = test_core_with_account();

    let file1 = {
        let id = core1.create_at_path("/test.md").unwrap().id;
        core1.sync(None).unwrap();
        core1.db.base_metadata.get(&id).unwrap().unwrap()
    };

    let mut file2 = file1.clone();
    file2.timestamped_value.value.document_hmac = Some([0; 32]);

    let result = api_service::request(
        &core2.get_account().unwrap(),
        ChangeDocRequest {
            diff: FileDiff::edit(&file1, &file2),
            new_content: AESEncrypted { value: vec![69], nonce: vec![69], _t: Default::default() },
        },
    );
    assert_matches!(
        result,
        Err(ApiError::<ChangeDocError>::Endpoint(ChangeDocError::DocumentNotFound))
    );
}

#[test]
fn get_someone_else_document() {
    let core1 = test_core_with_account();
    let core2 = test_core_with_account();

    let file = {
        let id = core1.create_at_path("/test.md").unwrap().id;
        core1.write_document(id, &[1, 2, 3]).unwrap();
        core1.sync(None).unwrap();
        core1.db.base_metadata.get(&id).unwrap().unwrap()
    };

    let req = GetDocRequest { id: *file.id(), hmac: *file.document_hmac().unwrap() };

    let result = api_service::request(&core2.get_account().unwrap(), req);
    assert_matches!(
        result,
        Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::DocumentNotFound)) // todo: not permissioned instead
    );
}
