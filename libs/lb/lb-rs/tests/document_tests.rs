use uuid::Uuid;

use lb_rs::io::docs::AsyncDocs;
use lb_rs::model::crypto::AESEncrypted;
use lb_rs::model::symkey;
use test_utils::{self, test_config};

#[tokio::test]
async fn get() {
    let config = &test_config();

    let id = Uuid::new_v4();
    let docs = AsyncDocs::from(config);
    let result = docs.get(id, Some(Default::default())).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn maybe_get() {
    let config = &test_config();

    let id = Uuid::new_v4();
    let docs = AsyncDocs::from(config);
    let result = docs.maybe_get(id, Some(Default::default())).await.unwrap();

    assert_eq!(result, None);
}

#[tokio::test]
async fn insert_get() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = AsyncDocs::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(id, Some(Default::default()), &document)
        .await
        .unwrap();
    let result = docs.get(id, Some(Default::default())).await.unwrap();

    assert_eq!(result, document);
}

#[tokio::test]
async fn insert_get_different_hmac() {
    let config = &test_config();
    let docs = AsyncDocs::from(config);
    let key = &symkey::generate_key();

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(
        id,
        Some([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ]),
        &document,
    )
    .await
    .unwrap();
    let result = docs
        .maybe_get(
            id,
            Some([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ]),
        )
        .await
        .unwrap();

    assert_eq!(result, None);
}

#[tokio::test]
async fn insert_get_overwrite_different_source() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = AsyncDocs::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(id, Some(Default::default()), &document)
        .await
        .unwrap();
    let (id_2, document_2) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_2").into_bytes()).unwrap());
    docs.insert(id_2, Some(Default::default()), &document_2)
        .await
        .unwrap();
    let result = docs.get(id, Some(Default::default())).await.unwrap();

    assert_eq!(result, document);
}

#[tokio::test]
async fn insert_get_all() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = AsyncDocs::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(id, Some(Default::default()), &document)
        .await
        .unwrap();
    let (id_2, document_2) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_2").into_bytes()).unwrap());
    docs.insert(id_2, Some(Default::default()), &document_2)
        .await
        .unwrap();
    let (id_3, document_3) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_3").into_bytes()).unwrap());
    docs.insert(id_3, Some(Default::default()), &document_3)
        .await
        .unwrap();
    let (id_4, document_4) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_4").into_bytes()).unwrap());
    docs.insert(id_4, Some(Default::default()), &document_4)
        .await
        .unwrap();
    let (id_5, document_5) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document_5").into_bytes()).unwrap());
    docs.insert(id_5, Some(Default::default()), &document_5)
        .await
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

#[tokio::test]
async fn insert_delete() {
    let config = &test_config();
    let key = &symkey::generate_key();
    let docs = AsyncDocs::from(config);

    let (id, document) =
        (Uuid::new_v4(), symkey::encrypt(key, &String::from("document").into_bytes()).unwrap());
    docs.insert(id, Some(Default::default()), &document)
        .await
        .unwrap();
    docs.delete(id, Some(Default::default())).await.unwrap();
    let result = docs.maybe_get(id, Some(Default::default())).await.unwrap();

    assert_eq!(result, None);
}
