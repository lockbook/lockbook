use lockbook_core::pure_functions::files;
use lockbook_core::service::file_encryption_service;
use lockbook_crypto::symkey;
use lockbook_models::file_metadata::FileType;
use lockbook_models::tree::{FileMetaMapExt, FileMetaVecExt};
use std::collections::HashMap;
use test_utils::*;
use uuid::Uuid;

#[test]
fn encrypt_decrypt_metadatum() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let key = symkey::generate_key();
    let file = files::create(FileType::Folder, Uuid::new_v4(), "folder", &account.public_key());

    let encrypted_file =
        file_encryption_service::encrypt_metadatum(&account, &account.public_key(), &key, &file)
            .unwrap();
    let decrypted_file = file_encryption_service::decrypt_metadatum(&key, &encrypted_file).unwrap();

    assert_eq!(file, decrypted_file);
}

#[test]
fn encrypt_decrypt_metadata() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Folder, folder.id, "document", &account.public_key());
    let files = [root.clone(), folder.clone(), document.clone()].to_map();

    let encrypted_files =
        file_encryption_service::encrypt_metadata(&account, &account.public_key(), &files).unwrap();
    let decrypted_files =
        file_encryption_service::decrypt_metadata(&account, &encrypted_files, &mut HashMap::new())
            .unwrap();

    assert_eq!(files.find(root.id).unwrap(), decrypted_files.find(root.id).unwrap(),);
    assert_eq!(files.find(folder.id).unwrap(), decrypted_files.find(folder.id).unwrap(),);
    assert_eq!(files.find(document.id).unwrap(), decrypted_files.find(document.id).unwrap(),);
}
