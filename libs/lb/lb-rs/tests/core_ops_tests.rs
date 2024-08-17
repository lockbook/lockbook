use lb_rs::shared::account::Account;
use lb_rs::shared::file_like::FileLike;
use lb_rs::shared::file_metadata::{FileMetadata, FileType};
use lb_rs::shared::symkey;
use lb_rs::shared::tree_like::TreeLike;
use test_utils::*;
use uuid::Uuid;

#[test]
fn test_create() {
    let account = &Account::new(random_name(), url());
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();
    let files = vec![root.clone()].to_lazy();
    let mut files = files.stage(vec![]);
    let id = files
        .create(
            Uuid::new_v4(),
            symkey::generate_key(),
            root.id(),
            "test-doc",
            FileType::Document,
            account,
        )
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
    let mut files = files.stage(vec![]);
    let id = files
        .create(
            Uuid::new_v4(),
            symkey::generate_key(),
            root.id(),
            "test-doc",
            FileType::Document,
            account,
        )
        .unwrap();

    files.rename(&id, "new-name", account).unwrap();

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
    let mut tree = tree.stage(vec![]);
    let doc = tree
        .create(
            Uuid::new_v4(),
            symkey::generate_key(),
            root.id(),
            "test-doc",
            FileType::Document,
            account,
        )
        .unwrap();
    let dir = tree
        .create(Uuid::new_v4(), symkey::generate_key(), root.id(), "dir", FileType::Folder, account)
        .unwrap();

    // Root should have 2 children and dir should have 0 child right now
    let children = tree.children(&dir).unwrap();
    assert_eq!(children.len(), 0);
    let children = tree.children(root.id()).unwrap();
    assert_eq!(children.len(), 2);

    tree.move_file(&doc, &dir, account).unwrap();

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
