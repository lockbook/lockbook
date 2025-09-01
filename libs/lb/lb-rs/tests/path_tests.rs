use lb_rs::model::account::Account;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::meta::Meta;
use lb_rs::model::tree_like::TreeLike;
use lb_rs::service::keychain::Keychain;
use test_utils::*;

#[tokio::test]
async fn test_create_path() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();

    let mut tree = vec![root.clone()].to_lazy().stage(vec![]);
    tree.create_at_path("test1", root.id(), &keychain).unwrap();
    tree.create_at_path("test2", root.id(), &keychain).unwrap();
    tree.create_at_path("test3", root.id(), &keychain).unwrap();

    let paths = tree.list_paths(None, &keychain).unwrap();
    let paths: Vec<String> = paths.into_iter().map(|(_, path)| path).collect();

    assert_eq!(paths.len(), 4);
    assert!(paths.contains(&"/".to_string()));
    assert!(paths.contains(&"/test1".to_string()));
    assert!(paths.contains(&"/test2".to_string()));
    assert!(paths.contains(&"/test3".to_string()));
}

#[tokio::test]
async fn test_path2() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();

    let mut tree = vec![root.clone()].to_lazy().stage(vec![]);
    tree.create_at_path("test1/2/3", root.id(), &keychain)
        .unwrap();

    let paths = tree.list_paths(None, &keychain).unwrap();
    let paths: Vec<String> = paths.into_iter().map(|(_, path)| path).collect();

    assert_eq!(paths.len(), 4);
    assert!(paths.contains(&"/".to_string()));
    assert!(paths.contains(&"/test1/".to_string()));
    assert!(paths.contains(&"/test1/2/".to_string()));
    assert!(paths.contains(&"/test1/2/3".to_string()));
}

#[tokio::test]
async fn test_path_to_id() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();

    let mut tree = vec![root.clone()].to_lazy().stage(vec![]);
    tree.create_at_path("test1/2/3", root.id(), &keychain)
        .unwrap();

    assert_eq!(tree.path_to_id("/", root.id(), &keychain).unwrap(), *root.id());

    let test1_id = tree.path_to_id("/test1", root.id(), &keychain).unwrap();
    assert_eq!(tree.name_using_links(&test1_id, &keychain).unwrap(), "test1");

    let two_id = tree.path_to_id("/test1/2", root.id(), &keychain).unwrap();
    assert_eq!(tree.name_using_links(&two_id, &keychain).unwrap(), "2");

    let three_id = tree.path_to_id("/test1/2/3", root.id(), &keychain).unwrap();
    assert_eq!(tree.name_using_links(&three_id, &keychain).unwrap(), "3");
}

#[tokio::test]
async fn test_path_file_types() {
    let account = &Account::new(random_name(), url());
    let keychain = Keychain::from(Some(account));
    let root = Meta::create_root(account).unwrap().sign(&keychain).unwrap();

    let mut tree = vec![root.clone()].to_lazy().stage(vec![]);
    tree.create_at_path("test1/2/3", root.id(), &keychain)
        .unwrap();

    assert_eq!(tree.path_to_id("/", root.id(), &keychain).unwrap(), *root.id());

    let test1_id = tree.path_to_id("/test1", root.id(), &keychain).unwrap();
    assert_eq!(tree.find(&test1_id).unwrap().file_type(), FileType::Folder);

    let two_id = tree.path_to_id("/test1/2", root.id(), &keychain).unwrap();
    assert_eq!(tree.find(&two_id).unwrap().file_type(), FileType::Folder);

    let three_id = tree.path_to_id("/test1/2/3", root.id(), &keychain).unwrap();
    assert_eq!(tree.find(&three_id).unwrap().file_type(), FileType::Document);
}
