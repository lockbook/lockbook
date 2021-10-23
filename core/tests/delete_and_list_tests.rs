mod integration_test;

#[cfg(test)]
mod delete_and_list_tests {
    use lockbook_core::service::account_service;
    use lockbook_core::service::path_service::{create_at_path, Filter};
    use lockbook_core::service::test_utils::generate_account;
    use lockbook_core::service::test_utils::test_config;
    use lockbook_core::{delete_file, list_paths, make_account, path};

    #[test]
    fn test_create_and_delete() {
        let db = test_config();
        let account = make_account!(db);
        let doc = create_at_path(&db, path!(account, "doc.md")).unwrap();
        assert_eq!(
            list_paths(&db, Some(Filter::LeafNodesOnly)).unwrap().len(),
            1
        );

        delete_file(&db, doc.id).unwrap();

        assert_eq!(
            list_paths(&db, Some(Filter::LeafNodesOnly)).unwrap().len(),
            0
        );
    }
}
