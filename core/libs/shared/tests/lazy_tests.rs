use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileMetadata, FileType};
use lockbook_shared::tree_like::Stagable;
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
