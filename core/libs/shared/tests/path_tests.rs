use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileMetadata, FileType};
use lockbook_shared::tree_like::TreeLike;
use test_utils::*;

#[test]
fn test_create_path() {
    let account = &Account::new(random_name(), url());
    let pk = &account.public_key();
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();

    let mut tree = vec![root.clone()]
        .to_lazy()
        .stage(vec![])
        .create_at_path("test1", root.id(), account, pk)
        .unwrap()
        .0
        .create_at_path("test2", root.id(), account, pk)
        .unwrap()
        .0
        .create_at_path("test3", root.id(), account, pk)
        .unwrap()
        .0;

    let paths = tree.list_paths(None, account).unwrap();
    assert_eq!(paths.len(), 4);
    assert!(paths.contains(&"/".to_string()));
    assert!(paths.contains(&"/test1".to_string()));
    assert!(paths.contains(&"/test2".to_string()));
    assert!(paths.contains(&"/test3".to_string()));
}

#[test]
fn test_path2() {
    let account = &Account::new(random_name(), url());
    let pk = &account.public_key();
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();

    let mut tree = vec![root.clone()]
        .to_lazy()
        .stage(vec![])
        .create_at_path("test1/2/3", root.id(), account, pk)
        .unwrap()
        .0;

    let paths = tree.list_paths(None, account).unwrap();

    assert_eq!(paths.len(), 4);
    assert!(paths.contains(&"/".to_string()));
    assert!(paths.contains(&"/test1/".to_string()));
    assert!(paths.contains(&"/test1/2/".to_string()));
    assert!(paths.contains(&"/test1/2/3".to_string()));
}

#[test]
fn test_path_to_id() {
    let account = &Account::new(random_name(), url());
    let pk = &account.public_key();
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();

    let mut tree = vec![root.clone()]
        .to_lazy()
        .stage(vec![])
        .create_at_path("test1/2/3", root.id(), account, pk)
        .unwrap()
        .0;

    assert_eq!(tree.path_to_id("/", root.id(), account), Ok(*root.id()));

    let test1_id = tree.path_to_id("/test1", root.id(), account).unwrap();
    assert_eq!(tree.name(&test1_id, account).unwrap(), "test1");

    let two_id = tree.path_to_id("/test1/2", root.id(), account).unwrap();
    assert_eq!(tree.name(&two_id, account).unwrap(), "2");

    let three_id = tree.path_to_id("/test1/2/3", root.id(), account).unwrap();
    assert_eq!(tree.name(&three_id, account).unwrap(), "3");
}

#[test]
fn test_path_file_types() {
    let account = &Account::new(random_name(), url());
    let pk = &account.public_key();
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();

    let mut tree = vec![root.clone()]
        .to_lazy()
        .stage(vec![])
        .create_at_path("test1/2/3", root.id(), account, pk)
        .unwrap()
        .0;

    assert_eq!(tree.path_to_id("/", root.id(), account), Ok(*root.id()));

    let test1_id = tree.path_to_id("/test1", root.id(), account).unwrap();
    assert_eq!(tree.find(&test1_id).unwrap().file_type(), FileType::Folder);

    let two_id = tree.path_to_id("/test1/2", root.id(), account).unwrap();
    assert_eq!(tree.find(&two_id).unwrap().file_type(), FileType::Folder);

    let three_id = tree.path_to_id("/test1/2/3", root.id(), account).unwrap();
    assert_eq!(tree.find(&three_id).unwrap().file_type(), FileType::Document);
}
