mod integration_test;

#[cfg(test)]
mod delete_and_list_tests {
    use lockbook_core::service::account_service;
    use lockbook_core::service::path_service::{create_at_path, Filter};
    use lockbook_core::service::test_utils::generate_account;
    use lockbook_core::service::test_utils::test_config;
    use lockbook_core::Error::UiError;
    use lockbook_core::{
        assert_matches, delete_file, list_paths, make_account, path, read_document, write_document,
        ReadDocumentError, WriteToDocumentError,
    };

    #[test]
    fn test_create_delete_list() {
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

    #[test]
    fn test_create_delete_read() {
        let db = test_config();
        let account = make_account!(db);
        let doc = create_at_path(&db, path!(account, "doc.md")).unwrap();
        assert_eq!(
            list_paths(&db, Some(Filter::LeafNodesOnly)).unwrap().len(),
            1
        );

        delete_file(&db, doc.id).unwrap();

        assert_matches!(
            read_document(&db, doc.id),
            Err(UiError(ReadDocumentError::FileDoesNotExist))
        );
    }

    #[test]
    fn test_create_delete_write() {
        let db = test_config();
        let account = make_account!(db);
        let doc = create_at_path(&db, path!(account, "doc.md")).unwrap();
        assert_eq!(
            list_paths(&db, Some(Filter::LeafNodesOnly)).unwrap().len(),
            1
        );

        delete_file(&db, doc.id).unwrap();

        assert_matches!(
            write_document(&db, doc.id, "document content".as_bytes()),
            Err(UiError(WriteToDocumentError::FileDoesNotExist))
        );
    }
}
