use lockbook_core::service::search_service::SearchResultItem;
use test_utils::*;

#[test]
fn test_matches() {
    let core = test_core_with_account();

    vec!["/abc.md", "/abcd.md", "/abcde.md", "/dir/doc1", "/dir/doc2", "/dir/doc3"]
        .into_iter()
        .for_each(|item| {
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
    assert_result_paths(&search_results, &["/abcd.md", "/abcde.md", "/abc.md"]);
}

fn assert_result_paths(results: &[SearchResultItem], paths: &[&str]) {
    assert_eq!(results.len(), paths.len());
    for i in 0..results.len() {
        assert_eq!(results.get(i).unwrap().path, *paths.get(i).unwrap());
    }
}
