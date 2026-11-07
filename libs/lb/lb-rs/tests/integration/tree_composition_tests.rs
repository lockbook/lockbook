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
async fn test_empty() {
    let account = Account::new(random_name(), url());
    let root = Meta::create_root(&account).unwrap();
    let files = vec![root].to_lazy().stage(vec![]);
    assert_eq!(files.tree.base.len(), 1);
    assert_eq!(files.tree.staged.len(), 0);
}

#[tokio::test]
async fn test_stage_promote() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();

    let mut files = vec![root.clone()].to_lazy().stage(vec![]);
    let (op, _) = files
        .create_op(
            Uuid::new_v4(),
            symkey::generate_key(),
            root.id(),
            "test",
            FileType::Folder,
            &keychain,
        )
        .unwrap();
    let files = files.tree.to_staged(Some(op)).to_lazy();

    assert_eq!(files.tree.base.base.len(), 1);
    assert_eq!(files.tree.base.staged.len(), 0);
    assert!(files.tree.staged.is_some());

    let files = files.promote().unwrap();
    assert_eq!(files.tree.base.len(), 1);
    assert_eq!(files.tree.staged.len(), 1);
}

#[tokio::test]
async fn test_stage_unstage() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();

    let mut files = vec![root.clone()].to_lazy().stage(vec![]);
    let (op, _) = files
        .create_op(
            Uuid::new_v4(),
            symkey::generate_key(),
            root.id(),
            "test",
            FileType::Folder,
            &keychain,
        )
        .unwrap();
    let files = files.tree.stage(Some(op)).to_lazy();

    assert_eq!(files.tree.base.base.len(), 1);
    assert_eq!(files.tree.base.staged.len(), 0);
    assert!(files.tree.staged.is_some());

    let files = files.unstage().0;
    assert_eq!(files.tree.base.len(), 1);
    assert_eq!(files.tree.staged.len(), 0);
}
