use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileMetadata, FileType};
use lockbook_shared::tree_like::Stagable;
use test_utils::*;

#[test]
fn test_empty() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let files = vec![root].to_lazy().stage(vec![]);
    assert_eq!(files.tree.base.len(), 1);
    assert_eq!(files.tree.staged.len(), 0);
}

#[test]
fn test_stage_promote() {
    let account = &Account::new(random_name(), url());
    let pk = &account.public_key();
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();
    let files = vec![root.clone()].to_lazy().stage(vec![]);
    let files = files
        .stage_create(root.id(), "test", FileType::Folder, account, pk)
        .unwrap()
        .0;

    assert_eq!(files.tree.base.base.len(), 1);
    assert_eq!(files.tree.base.staged.len(), 0);
    assert!(files.tree.staged.is_some());

    let files = files.promote();
    assert_eq!(files.tree.base.len(), 1);
    assert_eq!(files.tree.staged.len(), 1);
}

#[test]
fn test_stage_unstage() {
    let account = &Account::new(random_name(), url());
    let pk = &account.public_key();
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();
    let files = vec![root.clone()].to_lazy().stage(vec![]);
    let files = files
        .stage_create(root.id(), "test", FileType::Folder, account, pk)
        .unwrap()
        .0;

    assert_eq!(files.tree.base.base.len(), 1);
    assert_eq!(files.tree.base.staged.len(), 0);
    assert!(files.tree.staged.is_some());

    let files = files.unstage().0;
    assert_eq!(files.tree.base.len(), 1);
    assert_eq!(files.tree.staged.len(), 0);
}
