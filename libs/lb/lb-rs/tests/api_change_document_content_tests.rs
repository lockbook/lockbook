use lb_rs::logic::api::*;
use lb_rs::logic::crypto::AESEncrypted;
use lb_rs::logic::file_metadata::FileDiff;
use lb_rs::service::api_service::{ApiError, Requester};
use test_utils::assert_matches;
use test_utils::*;
use uuid::Uuid;

#[test]
fn change_document_content() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").unwrap().id;
    let doc = core
        .in_tx(|s| Ok(s.db.local_metadata.get().get(&doc).unwrap().clone()))
        .unwrap();

    // create document
    core.in_tx(|s| {
        s.client
            .request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] })
            .unwrap();
        Ok(())
    })
    .unwrap();

    let doc1 = doc;
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.document_hmac = Some([0; 32]);

    let diff = FileDiff::edit(&doc1, &doc2);
    // change document content
    core.in_tx(|s| {
        s.client
            .request(
                &account,
                ChangeDocRequest {
                    diff,
                    new_content: AESEncrypted {
                        value: vec![],
                        nonce: vec![],
                        _t: Default::default(),
                    },
                },
            )
            .unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn change_document_content_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").unwrap().id;
    let mut doc = core
        .in_tx(|s| Ok(s.db.local_metadata.get().get(&doc).unwrap().clone()))
        .unwrap();
    // create document
    core.in_tx(|s| {
        s.client
            .request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] })
            .unwrap();
        Ok(())
    })
    .unwrap();

    doc.timestamped_value.value.id = Uuid::new_v4();
    let doc1 = doc;
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.document_hmac = Some([0; 32]);

    let diff = FileDiff::edit(&doc1, &doc2);
    // change document content
    core.in_tx(|s| {
        let res = s.client.request(
            &account,
            ChangeDocRequest {
                diff,
                new_content: AESEncrypted { value: vec![], nonce: vec![], _t: Default::default() },
            },
        );
        assert_matches!(
            res,
            Err(ApiError::<ChangeDocError>::Endpoint(ChangeDocError::DocumentNotFound))
        );
        Ok(())
    })
    .unwrap()
}
