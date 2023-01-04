use lockbook_core::service::usage_service::UsageItemMetric;
use test_utils::*;

#[test]
fn get_uncompressed_usage_no_documents() {
    let core = &test_core_with_account();

    assert_eq!(
        core.get_uncompressed_usage().unwrap(),
        UsageItemMetric { exact: 0, readable: "0 B".to_string() }
    );
}

#[test]
fn get_uncompressed_usage_empty_document() {
    let core = &test_core_with_account();
    let document = core.create_at_path("document").unwrap();
    core.write_document(document.id, b"").unwrap();
    assert_eq!(
        core.get_uncompressed_usage().unwrap(),
        UsageItemMetric { exact: 0, readable: "0 B".to_string() }
    );
}

#[test]
fn get_uncompressed_usage_one_document() {
    let core = &test_core_with_account();
    let document = core.create_at_path("document").unwrap();

    core.write_document(document.id, b"0123456789").unwrap();

    assert_eq!(
        core.get_uncompressed_usage().unwrap(),
        UsageItemMetric { exact: 10, readable: "10 B".to_string() }
    );
}

#[test]
fn get_uncompressed_usage_multiple_documents() {
    let core = &test_core_with_account();
    let document1 = core.create_at_path("document1").unwrap();
    let document2 = core.create_at_path("document2").unwrap();

    core.write_document(document1.id, b"01234").unwrap();
    core.write_document(document2.id, b"56789").unwrap();

    assert_eq!(
        core.get_uncompressed_usage().unwrap(),
        UsageItemMetric { exact: 10, readable: "10 B".to_string() }
    );
}

#[test]
fn get_uncompressed_usage_with_delete() {
    let core = &test_core_with_account();
    let document1 = core.create_at_path("document1").unwrap();
    let document2 = core.create_at_path("document2").unwrap();

    core.write_document(document1.id, b"01234").unwrap();
    core.write_document(document2.id, b"56789").unwrap();

    core.delete_file(document2.id).unwrap();

    assert_eq!(
        core.get_uncompressed_usage().unwrap(),
        UsageItemMetric { exact: 5, readable: "5 B".to_string() }
    );
}

#[test]
fn get_uncompressed_usage_with_sync() {
    let core = &test_core_with_account();
    let document1 = core.create_at_path("document1").unwrap();
    let document2 = core.create_at_path("document2").unwrap();

    core.write_document(document1.id, b"01234").unwrap();
    core.write_document(document2.id, b"56789").unwrap();
    core.sync(None).unwrap();
    let core2 = test_core_from(core);

    assert_eq!(
        core2.get_uncompressed_usage().unwrap(),
        UsageItemMetric { exact: 10, readable: "10 B".to_string() }
    );
}
