use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileMetadata, FileType};
use lockbook_shared::tree_like::{Stagable, TreeLike};
use test_utils::*;

#[test]
fn decrypt_basic_name() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    assert_eq!(files.name(&root.id, &account).unwrap(), account.username);
}

#[test]
fn decrypt_child_name_basic() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child =
        FileMetadata::create(&account.public_key(), root.id, &key, "test", FileType::Document)
            .unwrap();
    let mut files = vec![root.clone(), child.clone()].to_lazy();
    assert_eq!(files.name(child.id(), &account).unwrap(), "test");
}

#[test]
fn decrypt_child_name_staged() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child =
        FileMetadata::create(&account.public_key(), root.id, &key, "test", FileType::Document)
            .unwrap();
    let mut files = files.stage(Some(child.clone()));
    assert_eq!(files.name(child.id(), &account).unwrap(), "test");
}

#[test]
fn decrypt_child_name_stage_promote() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child =
        FileMetadata::create(&account.public_key(), root.id, &key, "test", FileType::Document)
            .unwrap();
    let mut files = files.stage(Some(child.clone())).promote();
    assert_eq!(files.name(child.id(), &account).unwrap(), "test");
}

#[test]
fn decrypt_child_name_insert() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child =
        FileMetadata::create(&account.public_key(), root.id, &key, "test", FileType::Document)
            .unwrap();
    files.insert(child.clone());
    assert_eq!(files.name(child.id(), &account).unwrap(), "test");
}

#[test]
fn name_2dirs() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child =
        FileMetadata::create(&account.public_key(), root.id, &key, "dir1", FileType::Folder)
            .unwrap();
    files.insert(child.clone());
    let key = files.decrypt_key(child.id(), &account).unwrap();
    let child_of_child =
        FileMetadata::create(&account.public_key(), child.id, &key, "dir2", FileType::Folder)
            .unwrap();
    files.insert(child_of_child.clone());
    assert_eq!(files.name(root.id(), &account).unwrap(), account.username);
    assert_eq!(files.name(child.id(), &account).unwrap(), "dir1");
    assert_eq!(files.name(child_of_child.id(), &account).unwrap(), "dir2");
}

#[test]
fn deleted_2dirs() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let mut child =
        FileMetadata::create(&account.public_key(), root.id, &key, "dir1", FileType::Folder)
            .unwrap();
    child.is_deleted = true;
    files.insert(child.clone());
    let key = files.decrypt_key(child.id(), &account).unwrap();
    let child_of_child =
        FileMetadata::create(&account.public_key(), child.id, &key, "dir2", FileType::Folder)
            .unwrap();
    files.insert(child_of_child.clone());

    assert!(files.calculate_deleted(child.id()).unwrap());
    assert!(files.calculate_deleted(child_of_child.id()).unwrap());
}

#[test]
fn deleted_2dirs2() {
    let account = Account::new(random_name(), url());
    let root = FileMetadata::create_root(&account).unwrap();
    let mut files = vec![root.clone()].to_lazy();
    let key = files.decrypt_key(root.id(), &account).unwrap();
    let child =
        FileMetadata::create(&account.public_key(), root.id, &key, "dir1", FileType::Folder)
            .unwrap();
    files.insert(child.clone());
    let key = files.decrypt_key(child.id(), &account).unwrap();
    let mut child_of_child =
        FileMetadata::create(&account.public_key(), child.id, &key, "dir2", FileType::Folder)
            .unwrap();
    child_of_child.is_deleted = true;
    files.insert(child_of_child.clone());

    assert!(!files.calculate_deleted(child.id()).unwrap());
    assert!(files.calculate_deleted(child_of_child.id()).unwrap());
}
