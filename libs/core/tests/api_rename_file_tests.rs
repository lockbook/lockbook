use lockbook_core::service::api_service::Requester;
use lockbook_shared::api::*;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileDiff;
use test_utils::*;

#[test]
fn rename_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let doc = core.create_at_path("test.md").unwrap().id;
    let doc = core
        .in_tx(|s| {
            let doc = s.db.local_metadata.get().get(&doc).unwrap();
            s.client
                .request(&account, UpsertRequest { updates: vec![FileDiff::new(doc)] })
                .unwrap();

            Ok(doc.clone())
        })
        .unwrap();

    let old = doc.clone();
    core.rename_file(*doc.id(), &random_name()).unwrap();
    core.in_tx(|s| {
        let new = s.db.local_metadata.get().get(doc.id()).unwrap();

        s.client
            .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&old, new)] })
            .unwrap();
        Ok(())
    })
    .unwrap();
}
