use lockbook_core::{Core, Uuid};
use test_utils::*;

#[test]
fn base_case() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document = core.create_at_path("hello.md").unwrap();
    core.write_document(document.id, "hello world".as_bytes())
        .unwrap();

    let expected_suggestions = core.suggested_docs().unwrap();

    assert_eq!(vec![document.id], expected_suggestions);
}

#[test]
fn no_documents_suggestion() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let expected = core.suggested_docs().unwrap();
    let actual: Vec<Uuid> = vec![];

    assert_eq!(actual, expected);
}

#[test]
fn io_count_comparison() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello1.md").unwrap();
    for _ in 0..100 {
        core.write_document(document1.id, "hello world".as_bytes())
            .unwrap();
    }

    let document2 = core.create_at_path("hello2.md").unwrap();
    for _ in 0..50 {
        core.write_document(document2.id, "hello world".as_bytes())
            .unwrap();
    }

    let expected_suggestions = core.suggested_docs().unwrap();

    assert_eq!(vec![document1.id, document2.id], expected_suggestions);
}

#[test]
fn io_count_comparison_multiple_docs() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello.md").unwrap();
    for _ in 0..100 {
        core.write_document(document1.id, "hello world".as_bytes())
            .unwrap();
    }

    let document2 = core.create_at_path("hello2.md").unwrap();
    for _ in 0..20 {
        core.write_document(document2.id, "hello world".as_bytes())
            .unwrap();
    }

    let document3 = core.create_at_path("hello3.md").unwrap();
    for _ in 0..10 {
        core.write_document(document3.id, "hello world".as_bytes())
            .unwrap();
    }

    let expected_suggestions = core.suggested_docs().unwrap();
    assert_eq!(vec![document1.id, document2.id, document3.id], expected_suggestions);
}
