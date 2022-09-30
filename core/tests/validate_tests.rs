use hmdb::transaction::{Transaction, TransactionTable};
use lockbook_core::repo::schema::helper_log::{base_metadata, local_metadata};
use lockbook_shared::account::Account;
use lockbook_shared::file::File;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::{SharedError, SharedResult, ValidationFailure};
use test_utils::*;
use uuid::Uuid;

type BaseMetadata<'a> = TransactionTable<'a, Uuid, SignedFile, base_metadata>;
type LocalMetadata<'a> = TransactionTable<'a, Uuid, SignedFile, local_metadata>;

fn run(
    f: fn(Account, Owner, File, &mut BaseMetadata, &mut LocalMetadata) -> SharedResult<()>,
) -> SharedResult<()> {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let owner = Owner(account.public_key());
    let root = core.get_root().unwrap();
    core.db
        .transaction::<_, SharedResult<_>>(|tx| {
            f(account, owner, root, &mut tx.base_metadata, &mut tx.local_metadata)
        })
        .unwrap()
}

#[test]
fn create_two_files_with_same_path() {
    assert_matches!(
        run(|account, owner, root, base_metadata, local_metadata| {
            let tree = base_metadata.stage(local_metadata).to_lazy();

            let mut tree = tree.stage(Vec::new());
            let (staged_tree, _) = tree
                .stage_create(&root.id, "document", FileType::Document, &account)
                .unwrap();
            tree = staged_tree.promote();
            let (staged_tree, _) = tree
                .stage_create(&root.id, "document", FileType::Document, &account)
                .unwrap();
            tree = staged_tree.promote();
            tree.validate(owner)?.promote();

            Ok(())
        }),
        Err(SharedError::ValidationFailure(ValidationFailure::PathConflict(_)))
    );
}
