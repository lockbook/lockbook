use lb_rs::DocumentService;
use lb_rs::OnDiskDocuments;
use uuid::Uuid;

use lockbook_shared::crypto::AESEncrypted;
use lockbook_shared::symkey;
use test_utils::{self, test_config};

#[test]
fn get() {
    let config = &test_config();

    let id = Uuid::new_v4();
    let docs = OnDiskDocuments::from(config);
    let result = docs.get(&id, Some(&Default::default()));

    assert!(result.is_err());
}

#[test]
fn maybe_get() {
    let config = &test_config();

    let id = Uuid::new_v4();
    let docs = OnDiskDocuments::from(config);
    let result = docs.maybe_get(&id, Some(&Default::default())).unwrap();

    assert_eq!(result, None);
}

#[test]
fn insert_get() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = OnDiskDocuments::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(&id, Some(&Default::default()), &document)
        .unwrap();
    let result = docs.get(&id, Some(&Default::default())).unwrap();

    assert_eq!(result, document);
}

#[test]
fn insert_get_different_hmac() {
    let config = &test_config();
    let docs = OnDiskDocuments::from(config);
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(
        &id,
        Some(&[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ]),
        &document,
    )
    .unwrap();
    let result = docs
        .maybe_get(
            &id,
            Some(&[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ]),
        )
        .unwrap();

    assert_eq!(result, None);
}

#[test]
fn insert_get_overwrite_different_source() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = OnDiskDocuments::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(&id, Some(&Default::default()), &document)
        .unwrap();
    let (id_2, document_2) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_2").into_bytes()).unwrap());
    docs.insert(&id_2, Some(&Default::default()), &document_2)
        .unwrap();
    let result = docs.get(&id, Some(&Default::default())).unwrap();

    assert_eq!(result, document);
}

#[test]
fn insert_get_all() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = OnDiskDocuments::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(&id, Some(&Default::default()), &document)
        .unwrap();
    let (id_2, document_2) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_2").into_bytes()).unwrap());
    docs.insert(&id_2, Some(&Default::default()), &document_2)
        .unwrap();
    let (id_3, document_3) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_3").into_bytes()).unwrap());
    docs.insert(&id_3, Some(&Default::default()), &document_3)
        .unwrap();
    let (id_4, document_4) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_4").into_bytes()).unwrap());
    docs.insert(&id_4, Some(&Default::default()), &document_4)
        .unwrap();
    let (id_5, document_5) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_5").into_bytes()).unwrap());
    docs.insert(&id_5, Some(&Default::default()), &document_5)
        .unwrap();
    let result = test_utils::doc_repo_get_all(config);

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
fn insert_delete() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = OnDiskDocuments::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(&id, Some(&Default::default()), &document)
        .unwrap();
    docs.delete(&id, Some(&Default::default())).unwrap();
    let result = docs.maybe_get(&id, Some(&Default::default())).unwrap();

    assert_eq!(result, None);
}
