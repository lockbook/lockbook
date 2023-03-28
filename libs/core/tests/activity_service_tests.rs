use lockbook_core::{Core, Uuid};
use test_utils::*;

#[test]
fn base_case() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document = core.create_at_path("hello.md").unwrap();
    core.write_document(document.id, "hello world".as_bytes())
        .unwrap();

    let document1 = core.create_at_path("hello1.md").unwrap();
    core.write_document(document1.id, "hello world".as_bytes())
        .unwrap();

    let expected_suggestions = core.suggested_docs().unwrap();

    assert_eq!(vec![document1.id, document.id], expected_suggestions);
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

    let document = core.create_at_path("hello.md").unwrap();
    for _ in 0..100 {
        core.write_document(document.id, "hello world".as_bytes())
            .unwrap();
    }

    let document1 = core.create_at_path("hello1.md").unwrap();
    for _ in 0..100 {
        core.write_document(document1.id, "hello world".as_bytes())
            .unwrap();
    }

    let expected_suggestion = core.suggested_docs().unwrap()[0];

    assert_eq!(document.id, expected_suggestion);
}
