use lockbook_core::repo::local_storage;
use test_utils::test_config;

#[test]
fn read() {
    let db = &test_config();

    let result: Option<Vec<u8>> = local_storage::read(db, "namespace", "key").unwrap();

    assert_eq!(result, None);
}

#[test]
fn write_read() {
    let db = &test_config();

    local_storage::write(db, "namespace", "key", "value".as_bytes()).unwrap();
    let result: Vec<u8> = local_storage::read(db, "namespace", "key")
        .unwrap()
        .unwrap();

    assert_eq!(String::from_utf8_lossy(&result), "value");
}

#[test]
fn overwrite_read() {
    let db = &test_config();

    local_storage::write(db, "namespace", "key", "value-1".as_bytes()).unwrap();
    local_storage::write(db, "namespace", "key", "value-2".as_bytes()).unwrap();
    let result: Vec<u8> = local_storage::read(db, "namespace", "key")
        .unwrap()
        .unwrap();

    assert_eq!(String::from_utf8_lossy(&result), "value-2");
}

#[test]
fn delete() {
    let db = &test_config();

    local_storage::write(db, "namespace", "key-1", "value-1".as_bytes()).unwrap();
    local_storage::write(db, "namespace", "key-2", "value-2".as_bytes()).unwrap();
    local_storage::delete(db, "namespace", "key-2").unwrap();
    let result1: Vec<u8> = local_storage::read(db, "namespace", "key-1")
        .unwrap()
        .unwrap();
    let result2: Option<Vec<u8>> = local_storage::read(db, "namespace", "key-2").unwrap();

    assert_eq!(String::from_utf8_lossy(&result1), "value-1");
    assert_eq!(result2, None);
}

#[test]
fn delete_all() {
    let db = &test_config();

    local_storage::write(db, "namespace", "key-1", "value-1".as_bytes()).unwrap();
    local_storage::write(db, "namespace", "key-2", "value-2".as_bytes()).unwrap();
    local_storage::delete_all(db, "namespace").unwrap();
    let result1: Option<Vec<u8>> = local_storage::read(db, "namespace", "key-1").unwrap();
    let result2: Option<Vec<u8>> = local_storage::read(db, "namespace", "key-2").unwrap();

    assert_eq!(result1, None);
    assert_eq!(result2, None);
}

#[test]
fn delete_all_no_writes() {
    let db = &test_config();

    local_storage::delete_all(db, "namespace").unwrap();
}

#[test]
fn dump() {
    let db = &test_config();

    local_storage::write(db, "namespace", "key-1", "value-1".as_bytes()).unwrap();
    local_storage::write(db, "namespace", "key-4", "value-4".as_bytes()).unwrap();
    local_storage::write(db, "namespace", "key-3", "value-3".as_bytes()).unwrap();
    local_storage::write(db, "namespace", "key-2", "value-2".as_bytes()).unwrap();
    local_storage::write(db, "namespace", "key-5", "value-5".as_bytes()).unwrap();

    let result: Vec<Vec<u8>> = local_storage::dump(db, "namespace").unwrap();

    assert_eq!(
        result,
        vec![
            "value-1".as_bytes(),
            "value-2".as_bytes(),
            "value-3".as_bytes(),
            "value-4".as_bytes(),
            "value-5".as_bytes(),
        ]
    );
}
