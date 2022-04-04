#[cfg(test)]
mod search_tests {
    use lockbook_core::create_account;
    use lockbook_core::create_file_at_path;
    use lockbook_core::search_file_paths;
    use lockbook_core::service::search_service::SearchResultItem;
    use lockbook_core::service::test_utils;

    #[test]
    fn test_matches() {
        let config = test_utils::test_config();
        let generated_account = test_utils::generate_account();
        let username = generated_account.username;
        create_account(&config, &username, &generated_account.api_url).unwrap();

        vec!["abc.md", "abcd.md", "abcde.md", "dir/doc1", "dir/doc2", "dir/doc3"]
            .into_iter()
            .for_each(|path| {
                let path = format!("{}/{}", username, path);
                create_file_at_path(&config, &path).unwrap();
            });

        let search_results = search_file_paths(&config, "").unwrap();
        assert!(search_results.is_empty());

        let search_results = search_file_paths(&config, "abcde.md").unwrap();
        assert_result_paths(&search_results, &["/abcde.md"]);

        let search_results = search_file_paths(&config, "d/o").unwrap();
        assert_result_paths(&search_results, &["/dir/doc1", "/dir/doc2", "/dir/doc3"]);

        let search_results = search_file_paths(&config, "d/3").unwrap();
        assert_result_paths(&search_results, &["/dir/doc3"]);

        let search_results = search_file_paths(&config, "ad").unwrap();
        assert_result_paths(&search_results, &["/abcd.md", "/abcde.md", "/abc.md"]);
    }

    fn assert_result_paths(results: &[SearchResultItem], paths: &[&str]) {
        assert_eq!(results.len(), paths.len());
        for i in 0..results.len() {
            assert_eq!(results.get(i).unwrap().path, *paths.get(i).unwrap());
        }
    }
}
