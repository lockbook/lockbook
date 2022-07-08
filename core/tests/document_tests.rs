use lockbook_core::{Error, FileType, ShareMode, WriteToDocumentError};
use uuid::Uuid;

use lockbook_core::model::repo::RepoSource;
use lockbook_core::repo::document_repo;
use lockbook_crypto::symkey;
use lockbook_models::crypto::AESEncrypted;
use test_utils::*;

#[test]
fn get() {
    let config = &test_config();

    let id = Uuid::new_v4();
    let result = document_repo::get(config, RepoSource::Local, id);

    assert!(result.is_err());
}

#[test]
fn maybe_get() {
    let config = &test_config();

    let id = Uuid::new_v4();
    let result = document_repo::maybe_get(config, RepoSource::Local, id).unwrap();

    assert_eq!(result, None);
}

#[test]
fn insert_get() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    let result = document_repo::get(config, RepoSource::Local, id).unwrap();

    assert_eq!(result, document);
}

#[test]
fn insert_get_different_source() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    let result = document_repo::maybe_get(config, RepoSource::Base, id).unwrap();

    assert_eq!(result, None);
}

#[test]
fn insert_get_overwrite_different_source() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    let (id_2, document_2) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_2").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Base, id_2, &document_2).unwrap();
    let result = document_repo::get(config, RepoSource::Local, id).unwrap();

    assert_eq!(result, document);
}

#[test]
fn insert_get_all() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    let (id_2, document_2) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_2").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id_2, &document_2).unwrap();
    let (id_3, document_3) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_3").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id_3, &document_3).unwrap();
    let (id_4, document_4) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_4").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id_4, &document_4).unwrap();
    let (id_5, document_5) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_5").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id_5, &document_5).unwrap();
    let result = test_utils::doc_repo_get_all(config, RepoSource::Local);

    let mut expectation = vec![
        (id, document),
        (id_2, document_2),
        (id_3, document_3),
        (id_4, document_4),
        (id_5, document_5),
    ];
    expectation.sort_by(|(a, _), (b, _)| a.cmp(b));
    let expectation = expectation
        .into_iter()
        .map(|(_, d)| d)
        .collect::<Vec<AESEncrypted<Vec<u8>>>>();
    assert_eq!(result, expectation);
}

#[test]
fn insert_get_all_different_source() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    let result = test_utils::doc_repo_get_all(config, RepoSource::Base);

    assert_eq!(result, Vec::<AESEncrypted<Vec<u8>>>::new());
}

#[test]
fn insert_delete() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    document_repo::delete(config, RepoSource::Local, id).unwrap();
    let result = document_repo::maybe_get(config, RepoSource::Local, id).unwrap();

    assert_eq!(result, None);
}

#[test]
fn insert_delete_all() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    document_repo::delete_all(config, RepoSource::Local).unwrap();
    let result = test_utils::doc_repo_get_all(config, RepoSource::Local);

    assert_eq!(result, Vec::<AESEncrypted<Vec<u8>>>::new());
}

#[test]
fn insert_delete_all_different_source() {
    let config = &test_config();
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
    document_repo::delete_all(config, RepoSource::Base).unwrap();
    let result = test_utils::doc_repo_get_all(config, RepoSource::Local);

    assert_eq!(result, vec![document]);
}

#[test]
fn write_document_read_share() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document0 = cores[0]
        .create_file("document0", roots[0].id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(document0.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let result = cores[1].write_document(document0.id, b"document content");
    assert_matches!(result, Err(Error::UiError(WriteToDocumentError::InsufficientPermission)));
}

#[test]
fn write_document_write_share() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document0 = cores[0]
        .create_file("document0", roots[0].id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(document0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document0.id, b"document content")
        .unwrap();
}
