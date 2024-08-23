use lb_rs::logic::api::*;
use lb_rs::logic::crypto::AESEncrypted;
use lb_rs::service::api_service::{ApiError, Requester};

use lb_rs::logic::file_like::FileLike;
use lb_rs::logic::file_metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[test]
fn get_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").unwrap().id;
    core.sync(None).unwrap();
    core.in_tx(|s| {
        let old = s.db.base_metadata.get().get(&id).unwrap().clone();
        let mut new = old.clone();
        new.timestamped_value.value.document_hmac = Some([0; 32]);

        // update document content
        s.client
            .request(
                &account,
                ChangeDocRequest {
                    diff: FileDiff::edit(&old, &new),
                    new_content: AESEncrypted {
                        value: vec![69],
                        nonce: vec![69],
                        _t: Default::default(),
                    },
                },
            )
            .unwrap();

        // get document
        let result = s
            .client
            .request(&account, GetDocRequest { id, hmac: *new.document_hmac().unwrap() })
            .unwrap();
        assert_eq!(
            result.content,
            AESEncrypted { value: vec!(69), nonce: vec!(69), _t: Default::default() }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn get_document_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").unwrap().id;
    core.sync(None).unwrap();
    core.in_tx(|s| {
        let mut old = s.db.base_metadata.get().get(&id).unwrap().clone();
        old.timestamped_value.value.id = Uuid::new_v4();
        let mut new = old;
        new.timestamped_value.value.document_hmac = Some([0; 32]);

        // get document we never created
        let result = s.client.request(
            &account,
            GetDocRequest { id: *new.id(), hmac: *new.document_hmac().unwrap() },
        );
        assert_matches!(
            result,
            Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::DocumentNotFound))
        );
        Ok(())
    })
    .unwrap();
}
