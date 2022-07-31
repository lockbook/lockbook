use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileMetadata, FileType};
use lockbook_shared::tree_like::Stagable;
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

#[test]
fn test_rename() {
    let account = &Account::new(random_name(), url());
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();
    let files = vec![root.clone()].to_lazy();
    let files = files.stage(vec![]);
    let (files, id) = files
        .create(root.id(), "test-doc", FileType::Document, account, &account.public_key())
        .unwrap();

    let mut files = files.rename(&id, "new-name", account).unwrap();

    assert_eq!(files.name(&id, account).unwrap(), "new-name");
}

#[test]
fn test_move() {
    let account = &Account::new(random_name(), url());
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();

    // Create a tree with a doc and a dir
    let tree = vec![root.clone()].to_lazy();
    let tree = tree.stage(vec![]);
    let (tree, doc) = tree
        .create(root.id(), "test-doc", FileType::Document, account, &account.public_key())
        .unwrap();
    let (tree, dir) = tree
        .create(root.id(), "dir", FileType::Folder, account, &account.public_key())
        .unwrap();

    // Verify the starting state (dir is empty)
}
