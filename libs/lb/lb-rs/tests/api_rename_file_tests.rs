use lb_rs::model::api::*;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileDiff;
use test_utils::*;

#[tokio::test]
async fn rename_document() {
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
    core.client
        .request(account, UpsertRequest { updates: vec![FileDiff::new(doc.clone())] })
        .await
        .unwrap();

    let old = doc.clone();
    core.rename_file(doc.id(), &random_name()).await.unwrap();
    let new = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(doc.id())
        .unwrap()
        .clone();

    core.client
        .request(account, UpsertRequest { updates: vec![FileDiff::edit(old, new)] })
        .await
        .unwrap();
}
