use std::fs;
use std::path::Path;

use uuid::Uuid;

use lockbook_core::model::repo::RepoSource;
use lockbook_core::repo::{document_repo, local_storage};
use lockbook_core::Config;
use lockbook_crypto::symkey;
use lockbook_models::crypto::{AESEncrypted, EncryptedDocument};
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
    let result = doc_repo_get_all(config, RepoSource::Local);

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
    let result = doc_repo_get_all(config, RepoSource::Base);

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
    let result = doc_repo_get_all(config, RepoSource::Local);

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
    let result = doc_repo_get_all(config, RepoSource::Local);

    assert_eq!(result, vec![document]);
}

fn doc_repo_get_all(config: &Config, source: RepoSource) -> Vec<EncryptedDocument> {
    local_storage_dump::<_, Vec<u8>>(config, document_repo::namespace(source))
        .into_iter()
        .map(|s| bincode::deserialize(s.as_ref()).unwrap())
        .collect::<Vec<EncryptedDocument>>()
        .into_iter()
        .collect()
}

fn local_storage_dump<N, V>(db: &Config, namespace: N) -> Vec<V>
where
    N: AsRef<[u8]> + Copy,
    V: From<Vec<u8>>,
{
    let path_str = local_storage::namespace_path(db, namespace);
    let path = Path::new(&path_str);

    match fs::read_dir(path) {
        Ok(rd) => {
            let mut file_names = rd
                .map(|dir_entry| dir_entry.unwrap().file_name().into_string().unwrap())
                .collect::<Vec<String>>();
            file_names.sort();

            file_names
                .iter()
                .map(|file_name| {
                    local_storage::read(db, namespace, file_name)
                        .unwrap()
                        .unwrap()
                })
                .collect::<Vec<V>>()
        }
        Err(_) => Vec::new(),
    }
}
