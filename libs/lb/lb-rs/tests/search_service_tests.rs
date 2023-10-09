use crossbeam::channel::{Receiver, Sender};
use lb_rs::service::search_service::{SearchRequest, SearchResult, SearchResultItem};
use lockbook_shared::file::ShareMode;
use lockbook_shared::file_metadata::FileType;
use std::collections::HashSet;
use test_utils::*;

const FILE_PATHS: [&str; 6] =
    ["/abc.md", "/abcd.md", "/abcde.md", "/dir/doc1", "/dir/doc2", "/dir/doc3"];
const CONTENT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus \
lorem purus, malesuada a dui a, auctor lobortis dolor. Proin ut placerat lectus. Vestibulum massa \
orci, fermentum id nunc sit amet, scelerisque tempus enim. Duis tristique imperdiet ex. Curabitur \
sagittis augue vel orci eleifend, sed cursus ante porta. Phasellus pellentesque vulputate ante id \
fringilla. Suspendisse eu volutpat augue. Mauris massa nisl, venenatis eget viverra non, ultrices \
vel enim.";

const MATCHED_CONTENT_1: (&str, &str) = (
    "consectetur adipiscing elit. Vivamus lorem purus, malesuada",
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus lorem purus, \
malesuada a dui a, auctor lobortis dolor. Proin ut placerat lectus. Vestibulum m...",
);

const MATCHED_CONTENT_2: (&str, &str) = (
    "Mauris massa nisl, venenatis eget viverra",
    ".... Phasellus pellentesque vulputate ante id fringilla. Suspendisse eu volutpat augue. \
    Mauris massa nisl, venenatis eget viverra non, ultrices vel enim.",
);

const MATCHED_CONTENT_3: (&str, &str) = (
    "scelerisque tempus",
    "...t amet, scelerisque tempus enim. Duis tristique imperdiet ex. Curabitur sagittis augue \
    vel orci eleifend, sed cursus ante porta. Phasellus pellente...",
);

#[test]
fn test_matches() {
    let core = test_core_with_account();

    FILE_PATHS.iter().for_each(|item| {
        core.create_at_path(item).unwrap();
    });

    let search_results = core.search_file_paths("").unwrap();
    assert!(search_results.is_empty());

    let search_results = core.search_file_paths("abcde.md").unwrap();
    assert_result_paths(&search_results, &["/abcde.md"]);

    let search_results = core.search_file_paths("d/o").unwrap();
    assert_result_paths(&search_results, &["/dir/doc1", "/dir/doc2", "/dir/doc3"]);

    let search_results = core.search_file_paths("d/3").unwrap();
    assert_result_paths(&search_results, &["/dir/doc3"]);

    let search_results = core.search_file_paths("ad").unwrap();
    assert_result_paths(&search_results, &["/abc.md", "/abcd.md", "/abcde.md"]);
}

fn assert_result_paths(results: &[SearchResultItem], paths: &[&str]) {
    assert_eq!(results.len(), paths.len());
    for i in 0..results.len() {
        assert_eq!(results.get(i).unwrap().path, *paths.get(i).unwrap());
    }
}

fn assert_async_results_path(results: Vec<SearchResult>, paths: Vec<&str>) {
    assert_eq!(results.len(), paths.len());

    let results_set: HashSet<&str> = results
        .iter()
        .map(|result| match result {
            SearchResult::FileNameMatch { path, .. } => path.as_str(),
            _ => panic!("Non file name match, search_result: {:?}", result),
        })
        .collect();
    let paths_set: HashSet<&str> = paths.into_iter().collect();

    assert_eq!(results_set, paths_set)
}

#[test]
fn test_async_name_matches() {
    let core = test_core_with_account();

    FILE_PATHS.iter().for_each(|item| {
        core.create_at_path(item).unwrap();
    });

    let start_search = core.start_search().unwrap();

    start_search
        .search_tx
        .send(SearchRequest::Search { input: "".to_string() })
        .unwrap();
    let result = start_search.results_rx.recv().unwrap();
    match result {
        SearchResult::NoMatch => {}
        _ => panic!("There should be no matched search results, search_result: {:?}", result),
    }

    start_search
        .search_tx
        .send(SearchRequest::Search { input: "a".to_string() })
        .unwrap();
    let results = vec![
        start_search.results_rx.recv().unwrap(),
        start_search.results_rx.recv().unwrap(),
        start_search.results_rx.recv().unwrap(),
    ];
    assert_async_results_path(results, vec!["/abc.md", "/abcd.md", "/abcde.md"]);

    start_search
        .search_tx
        .send(SearchRequest::Search { input: "dir".to_string() })
        .unwrap();
    let results = vec![
        start_search.results_rx.recv().unwrap(),
        start_search.results_rx.recv().unwrap(),
        start_search.results_rx.recv().unwrap(),
    ];
    assert_async_results_path(results, vec!["/dir/doc1", "/dir/doc2", "/dir/doc3"]);

    start_search
        .search_tx
        .send(SearchRequest::EndSearch)
        .unwrap();
}

fn assert_async_content_match(
    search_tx: &Sender<SearchRequest>, results_rx: &Receiver<SearchResult>, input: &str,
    matched_text: &str,
) {
    search_tx
        .send(SearchRequest::Search { input: input.to_string() })
        .unwrap();
    let result = results_rx.recv().unwrap();

    match result {
        SearchResult::FileContentMatches { content_matches, .. } => {
            assert_eq!(content_matches[0].paragraph, matched_text)
        }
        _ => panic!("There should be a content match, search_result: {:?}", result),
    }
}

#[test]
fn test_async_content_matches() {
    let core = test_core_with_account();

    let file = core.create_at_path("/aaaaaaaaaa.md").unwrap();
    core.write_document(file.id, CONTENT.as_bytes()).unwrap();

    let start_search = core.start_search().unwrap();

    assert_async_content_match(
        &start_search.search_tx,
        &start_search.results_rx,
        MATCHED_CONTENT_1.0,
        MATCHED_CONTENT_1.1,
    );
    assert_async_content_match(
        &start_search.search_tx,
        &start_search.results_rx,
        MATCHED_CONTENT_2.0,
        MATCHED_CONTENT_2.1,
    );
    assert_async_content_match(
        &start_search.search_tx,
        &start_search.results_rx,
        MATCHED_CONTENT_3.0,
        MATCHED_CONTENT_3.1,
    );

    start_search
        .search_tx
        .send(SearchRequest::EndSearch)
        .unwrap();
}

#[test]
fn test_pending_share_search() {
    let core1 = test_core_with_account();

    let core2 = test_core_with_account();

    let file1 = core1.create_at_path("/aaaaaaa.md").unwrap();
    core1.write_document(file1.id, CONTENT.as_bytes()).unwrap();
    core1
        .share_file(file1.id, &core2.get_account().unwrap().username, ShareMode::Read)
        .unwrap();

    core1.sync(None).unwrap();
    core2.sync(None).unwrap();

    core2.create_at_path("/bbbbbbb.md").unwrap();
    core2
        .create_file(&file1.name, core2.get_root().unwrap().id, FileType::Link { target: file1.id })
        .unwrap();

    let start_search = core2.start_search().unwrap();

    start_search
        .search_tx
        .send(SearchRequest::Search { input: "bbbb".to_string() })
        .unwrap();
    let results = vec![start_search.results_rx.recv().unwrap()];
    assert_async_results_path(results, vec!["/bbbbbbb.md"]);

    start_search
        .search_tx
        .send(SearchRequest::EndSearch)
        .unwrap();

    let search_results = core2.search_file_paths("bbb").unwrap();
    assert_result_paths(&search_results, &["/bbbbbbb.md"]);
}
