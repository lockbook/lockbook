use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileMetadata, FileType};
use lockbook_shared::tree_like::{Stagable, TreeLike};
use test_utils::*;

#[test]
fn test_create() {
    let account = &Account::new(random_name(), url());
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();
    let files = vec![root.clone()].to_lazy();
    let files = files.stage(vec![]);
    let (mut files, id) = files
        .create(root.id(), "test-doc", FileType::Document, account, &account.public_key())
        .unwrap();

    assert_eq!(files.name(&id, account).unwrap(), "test-doc");
}
