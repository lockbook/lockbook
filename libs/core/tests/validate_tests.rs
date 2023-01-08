use hmdb::transaction::Transaction;
use lockbook_shared::access_info::{UserAccessInfo, UserAccessMode};
use lockbook_shared::file::ShareMode;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::{symkey, SharedError, SharedResult, ValidationFailure};
use test_utils::*;
use uuid::Uuid;

#[test]
fn create_two_files_with_same_path() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let result = core
        .db
        .transaction::<_, SharedResult<_>>(|tx| {
            let tree = tx.base_metadata.stage(&mut tx.local_metadata).to_lazy();
            let mut tree = tree.stage(Vec::new());
            tree.create_unvalidated(
                Uuid::new_v4(),
                symkey::generate_key(),
                &root.id,
                "document",
                FileType::Document,
                &account,
            )
            .unwrap();
            tree.create_unvalidated(
                Uuid::new_v4(),
                symkey::generate_key(),
                &root.id,
                "document",
                FileType::Document,
                &account,
            )
            .unwrap();
            tree.validate(Owner(account.public_key()))?;

            Ok(())
        })
        .unwrap();

    assert_matches!(
        result,
        Err(SharedError::ValidationFailure(ValidationFailure::PathConflict(_)))
    );
}

#[test]
fn directly_shared_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let shared_doc = cores[0].create_at_path("/shared-doc").unwrap();
    cores[0]
        .share_file(shared_doc.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let link = cores[1]
        .create_link_at_path("/link", shared_doc.id)
        .unwrap();

    let result = cores[1]
        .db
        .transaction::<_, SharedResult<_>>(|tx| {
            // probably for the best that this is how ugly the code has to get to produce this situation
            let mut link = tx
                .local_metadata
                .get(&link.id)
                .unwrap()
                .timestamped_value
                .value
                .clone();
            link.user_access_keys.push(
                UserAccessInfo::encrypt(
                    &accounts[1],
                    &accounts[1].public_key(),
                    &accounts[0].public_key(),
                    &symkey::generate_key(),
                    UserAccessMode::Write,
                )
                .unwrap(),
            );
            tx.local_metadata
                .insert(link.id, link.sign(&accounts[1]).unwrap());

            let mut tree = tx.base_metadata.stage(&mut tx.local_metadata).to_lazy();
            tree.validate(Owner(accounts[1].public_key()))?;

            Ok(())
        })
        .unwrap();

    assert_matches!(
        result,
        Err(SharedError::ValidationFailure(ValidationFailure::SharedLink { .. }))
    );
}
