use std::{thread::sleep, time::Duration};

use lockbook_core::{Core, Uuid};
use lockbook_shared::document_repo::RankingWeights;
use test_utils::*;

#[test]
fn suggest_docs() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document = core.create_at_path("hello.md").unwrap();
    core.write_document(document.id, "hello world".as_bytes())
        .unwrap();

    let expected_suggestions = core.suggested_docs(None).unwrap();

    assert_eq!(vec![document.id], expected_suggestions);
}

#[test]
fn suggest_docs_empty() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let expected = core.suggested_docs(None).unwrap();
    let actual: Vec<Uuid> = vec![];

    assert_eq!(actual, expected);
}

#[test]
fn write_count() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello1.md").unwrap();
    for _ in 0..10 {
        core.write_document(document1.id, "hello world".as_bytes())
            .unwrap();
    }

    let document2 = core.create_at_path("hello2.md").unwrap();
    for _ in 0..20 {
        core.write_document(document2.id, "hello world".as_bytes())
            .unwrap();
    }

    let actual_suggestions = core
        .suggested_docs(Some(RankingWeights { temporality: 0, io: 100 }))
        .unwrap();
    let expected_suggestions = vec![document2.id, document1.id];
    assert_eq!(actual_suggestions, expected_suggestions);
}

#[test]
fn write_count_multiple_docs() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello.md").unwrap();
    for _ in 0..10 {
        core.write_document(document1.id, "hello world".as_bytes())
            .unwrap();
    }

    let document2 = core.create_at_path("hello2.md").unwrap();
    for _ in 0..50 {
        core.write_document(document2.id, "hello world".as_bytes())
            .unwrap();
    }

    let document3 = core.create_at_path("hello3.md").unwrap();
    for _ in 0..55 {
        core.write_document(document3.id, "hello world".as_bytes())
            .unwrap();
    }

    let actual_suggestions = core
        .suggested_docs(Some(RankingWeights { temporality: 0, io: 100 }))
        .unwrap();

    let expected_suggestions = vec![document3.id, document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}

#[test]
fn read_count() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello1.md").unwrap();
    for _ in 0..10 {
        core.read_document(document1.id).unwrap();
    }

    let document2 = core.create_at_path("hello2.md").unwrap();
    for _ in 0..20 {
        core.read_document(document2.id).unwrap();
    }

    let actual_suggestions = core
        .suggested_docs(Some(RankingWeights { temporality: 0, io: 100 }))
        .unwrap();
    let expected_suggestions = vec![document2.id, document1.id];
    assert_eq!(actual_suggestions, expected_suggestions);
}

#[test]
fn read_count_multiple_docs() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello.md").unwrap();
    for _ in 0..10 {
        core.read_document(document1.id).unwrap();
    }

    let document2 = core.create_at_path("hello2.md").unwrap();
    for _ in 0..20 {
        core.read_document(document2.id).unwrap();
    }

    let document3 = core.create_at_path("hello3.md").unwrap();
    for _ in 0..100 {
        core.read_document(document3.id).unwrap();
    }

    let actual_suggestions = core
        .suggested_docs(Some(RankingWeights { temporality: 0, io: 100 }))
        .unwrap();

    let expected_suggestions = vec![document3.id, document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}

#[test]
fn last_read() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello.md").unwrap();
    core.read_document(document1.id).unwrap();

    sleep(Duration::from_millis(1000));

    let document2 = core.create_at_path("hello2.md").unwrap();
    core.read_document(document2.id).unwrap();

    let actual_suggestions = core
        .suggested_docs(Some(RankingWeights { temporality: 100, io: 0 }))
        .unwrap();

    let expected_suggestions = vec![document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}

#[test]
fn last_write() {
    let core: Core = test_core();
    core.create_account(&random_name(), &url(), false).unwrap();

    let document1 = core.create_at_path("hello.md").unwrap();
    core.write_document(document1.id, "hello world".as_bytes())
        .unwrap();

    sleep(Duration::from_millis(1000));

    let document2 = core.create_at_path("hello2.md").unwrap();
    core.write_document(document2.id, "hello world".as_bytes())
        .unwrap();

    let actual_suggestions = core
        .suggested_docs(Some(RankingWeights { temporality: 100, io: 0 }))
        .unwrap();

    let expected_suggestions = vec![document2.id, document1.id];

    assert_eq!(actual_suggestions, expected_suggestions);
}
