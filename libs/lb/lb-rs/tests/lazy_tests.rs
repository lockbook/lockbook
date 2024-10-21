use lb_rs::logic::file_like::FileLike;
use lb_rs::logic::symkey;
use lb_rs::logic::tree_like::TreeLike;
use lb_rs::model::account::Account;
use lb_rs::model::file_metadata::{FileMetadata, FileType};
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn decrypt_basic_name() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    assert_eq!(files.name_using_links(&root.id, &account).unwrap(), account.username);
}

#[tokio::test]
async fn decrypt_child_name_basic() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        root.id,
        &key,
        "test",
        FileType::Document,
    )
    .unwrap();
    let mut files = vec![root.clone(), child.clone()].to_lazy();
    assert_eq!(files.name_using_links(child.id(), &account).unwrap(), "test");
}

#[tokio::test]
async fn decrypt_child_name_staged() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        root.id,
        &key,
        "test",
        FileType::Document,
    )
    .unwrap();
    let mut files = files.stage(Some(child.clone()));
    assert_eq!(files.name_using_links(child.id(), &account).unwrap(), "test");
}

#[tokio::test]
async fn decrypt_child_name_stage_promote() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        root.id,
        &key,
        "test",
        FileType::Document,
    )
    .unwrap();
    let mut files = files.stage(Some(child.clone())).promote().unwrap();
    assert_eq!(files.name_using_links(child.id(), &account).unwrap(), "test");
}

#[tokio::test]
async fn decrypt_child_name_insert() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        root.id,
        &key,
        "test",
        FileType::Document,
    )
    .unwrap();
    files = files.stage(Some(child.clone())).promote().unwrap();
    assert_eq!(files.name_using_links(child.id(), &account).unwrap(), "test");
}

#[tokio::test]
async fn name_2dirs() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        root.id,
        &key,
        "dir1",
        FileType::Folder,
    )
    .unwrap();
    files = files.stage(Some(child.clone())).promote().unwrap();
    let key = files.decrypt_key(child.id(), &account).unwrap();
    let child_of_child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        child.id,
        &key,
        "dir2",
        FileType::Folder,
    )
    .unwrap();
    files = files.stage(Some(child_of_child.clone())).promote().unwrap();
    assert_eq!(files.name_using_links(root.id(), &account).unwrap(), account.username);
    assert_eq!(files.name_using_links(child.id(), &account).unwrap(), "dir1");
    assert_eq!(
        files
            .name_using_links(child_of_child.id(), &account)
            .unwrap(),
        "dir2"
    );
}

#[tokio::test]
async fn deleted_2dirs() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let mut child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        root.id,
        &key,
        "dir1",
        FileType::Folder,
    )
    .unwrap();
    child.is_deleted = true;
    files = files.stage(Some(child.clone())).promote().unwrap();
    let key = files.decrypt_key(child.id(), &account).unwrap();
    let child_of_child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        child.id,
        &key,
        "dir2",
        FileType::Folder,
    )
    .unwrap();
    files = files.stage(Some(child_of_child.clone())).promote().unwrap();

    assert!(files.calculate_deleted(child.id()).unwrap());
    assert!(files.calculate_deleted(child_of_child.id()).unwrap());
}

#[tokio::test]
async fn deleted_2dirs2() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        root.id,
        &key,
        "dir1",
        FileType::Folder,
    )
    .unwrap();
    files = files.stage(Some(child.clone())).promote().unwrap();
    let key = files.decrypt_key(child.id(), &account).unwrap();
    let mut child_of_child = FileMetadata::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &account.public_key(),
        child.id,
        &key,
        "dir2",
        FileType::Folder,
    )
    .unwrap();
    child_of_child.is_deleted = true;
    files = files.stage(Some(child_of_child.clone())).promote().unwrap();

    assert!(!files.calculate_deleted(child.id()).unwrap());
    assert!(files.calculate_deleted(child_of_child.id()).unwrap());
}
