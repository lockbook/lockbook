use lb_rs::model::account::Account;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::meta::Meta;
use lb_rs::model::symkey;
use lb_rs::model::tree_like::TreeLike;
use lb_rs::service::keychain::Keychain;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn test_create() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account)
        .unwrap()
        .sign_with(account)
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
            &keychain,
        )
        .unwrap();

    assert_eq!(files.name(&id, &keychain).unwrap(), "test-doc");
}

#[tokio::test]
async fn test_rename() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();
    let files = vec![root.clone()].to_lazy();
    let mut files = files.stage(vec![]);
    let id = files
        .create(
            Uuid::new_v4(),
            symkey::generate_key(),
            root.id(),
            "test-doc",
            FileType::Document,
            &keychain,
        )
        .unwrap();

    files.rename(&id, "new-name", &keychain).unwrap();

    assert_eq!(files.name(&id, &keychain).unwrap(), "new-name");
}

#[tokio::test]
async fn test_children_and_move() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();

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
            &keychain,
        )
        .unwrap();
    let dir = tree
        .create(
            Uuid::new_v4(),
            symkey::generate_key(),
            root.id(),
            "dir",
            FileType::Folder,
            &keychain,
        )
        .unwrap();

    // Root should have 2 children and dir should have 0 child right now
    let children = tree.children(&dir).unwrap();
    assert_eq!(children.len(), 0);
    let children = tree.children(root.id()).unwrap();
    assert_eq!(children.len(), 2);

    tree.move_file(&doc, &dir, &keychain).unwrap();

    // Dir should have 1 child after the move
    let children = tree.children(&dir).unwrap();
    assert_eq!(children.len(), 1);
    assert!(children.contains(&doc));

    // Doc should have no children (obviously)
    let children = tree.children(&doc).unwrap();
    assert_eq!(children.len(), 0);

    // Root should have 1 child now
    let children = tree.children(root.id()).unwrap();
    assert_eq!(children.len(), 1);
}
