
use lockbook_core::{Core, Uuid};
use test_utils::*;

#[test]
fn suggest_docs() {
    let core: Core = test_core();

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

    let expected = core.suggested_docs().unwrap();
    let actual: Vec<Uuid> = vec![];

    assert_eq!(actual, expected);
}
