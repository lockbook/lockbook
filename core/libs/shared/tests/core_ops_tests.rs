use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileMetadata, FileType};
use lockbook_shared::lazy::LazyTreeLike;
use lockbook_shared::tree_like::TreeLikeMut;
use test_utils::*;

#[test]
fn test_create() {
    let account = &Account::new(random_name(), url());
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();
    let files = vec![root.clone()].to_lazy();
    let files = files.stage_lazy(vec![]);
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
    let files = files.stage_lazy(vec![]);
    let (files, id) = files
        .create(root.id(), "test-doc", FileType::Document, account, &account.public_key())
        .unwrap();

    let mut files = files.rename(&id, "new-name", account).unwrap();

    assert_eq!(files.name(&id, account).unwrap(), "new-name");
}

#[test]
fn test_children_and_move() {
    let account = &Account::new(random_name(), url());
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();

    // Create a tree with a doc and a dir
    let tree = vec![root.clone()].to_lazy();
    let tree = tree.stage_lazy(vec![]);
    let (tree, doc) = tree
        .create(root.id(), "test-doc", FileType::Document, account, &account.public_key())
        .unwrap();
    let (mut tree, dir) = tree
        .create(root.id(), "dir", FileType::Folder, account, &account.public_key())
        .unwrap();

    // Root should have 2 children and dir should have 0 child right now
    let children = tree.children(&dir).unwrap();
    assert_eq!(children.len(), 0);
    let children = tree.children(root.id()).unwrap();
    assert_eq!(children.len(), 2);

    let mut tree = tree.move_file(&doc, &dir, account).unwrap();

    // Dir should have 1 child after the move
    let children = tree.children(&dir).unwrap();
    assert_eq!(children.len(), 1);
    assert!(children.get(&doc).is_some());

    // Doc should have no children (obviously)
    let children = tree.children(&doc).unwrap();
    assert_eq!(children.len(), 0);

    // Root should have 1 child now
    let children = tree.children(root.id()).unwrap();
    assert_eq!(children.len(), 1);
}
