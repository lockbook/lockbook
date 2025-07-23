use lb_rs::model::access_info::{UserAccessInfo, UserAccessMode};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::{FileType, Owner};
use lb_rs::model::tree_like::TreeLike;
use lb_rs::model::{ValidationFailure, symkey};
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn create_two_files_with_same_path() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let root = core.root().await.unwrap();

    let mut tx = core.begin_tx().await;
    let db = tx.db();

    let tree = db.base_metadata.stage(&mut db.local_metadata).to_lazy();
    let mut tree = tree.stage(Vec::new());
    tree.create_unvalidated(
        Uuid::new_v4(),
        symkey::generate_key(),
        &root.id,
        "document",
        FileType::Document,
        &core.keychain,
    )
    .unwrap();
    tree.create_unvalidated(
        Uuid::new_v4(),
        symkey::generate_key(),
        &root.id,
        "document",
        FileType::Document,
        &core.keychain,
    )
    .unwrap();
    let result = tree.validate(Owner(account.public_key()));
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::PathConflict(_))
    );
}

#[tokio::test]
async fn directly_shared_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let shared_doc = cores[0].create_at_path("/shared-doc").await.unwrap();
    cores[0]
        .share_file(shared_doc.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_link_at_path("/link", shared_doc.id)
        .await
        .unwrap();

    // probably for the best that this is how ugly the code has to get to produce this situation
    let mut tx = cores[1].begin_tx().await;
    let mut link = tx
        .db()
        .local_metadata
        .get()
        .get(&link.id)
        .unwrap()
        .timestamped_value
        .value
        .clone();
    link.user_access_keys.push(
        UserAccessInfo::encrypt(
            accounts[1],
            &accounts[1].public_key(),
            &accounts[0].public_key(),
            &symkey::generate_key(),
            UserAccessMode::Write,
        )
        .unwrap(),
    );
    tx.db()
        .local_metadata
        .insert(link.id, link.sign(&cores[1].keychain).unwrap())
        .unwrap();

    let db = tx.db();
    let mut tree = db.base_metadata.stage(&mut db.local_metadata).to_lazy();
    let result = tree.validate(Owner(accounts[1].public_key()));
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::SharedLink { .. })
    );
}
