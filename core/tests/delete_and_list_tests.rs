mod integration_test;

#[cfg(test)]
mod delete_and_list_tests {
    use lockbook_core::service::account_service;
    use lockbook_core::service::path_service::Filter;
    use lockbook_core::service::test_utils::generate_account;
    use lockbook_core::service::test_utils::test_config;
    use lockbook_core::Error::UiError;
    use lockbook_models::file_metadata::FileType;

    use lockbook_core::{
        assert_matches, create_file, create_file_at_path, delete_file, get_root, list_paths,
        make_account, path, read_document, save_document_to_disk, write_document, CreateFileError,
        FileDeleteError, ReadDocumentError, SaveDocumentToDiskError, WriteToDocumentError,
    };

    #[test]
    fn test_create_delete_list() {
        let db = test_config();
        let account = make_account!(db);
        let doc = create_file_at_path(&db, path!(account, "doc.md")).unwrap();
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
        let doc = create_file_at_path(&db, path!(account, "doc.md")).unwrap();
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
        let doc = create_file_at_path(&db, path!(account, "doc.md")).unwrap();
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

    #[test]
    fn test_create_parent_delete_create_in_parent() {
        let db = test_config();
        let account = make_account!(db);
        let folder = create_file_at_path(&db, path!(account, "folder/")).unwrap();

        assert_eq!(
            list_paths(&db, Some(Filter::LeafNodesOnly)).unwrap().len(),
            1
        );

        delete_file(&db, folder.id).unwrap();

        assert_matches!(
            create_file(&db, "document", folder.id, FileType::Document),
            Err(UiError(CreateFileError::CouldNotFindAParent))
        );
    }

    #[test]
    fn try_to_delete_root() {
        let db = test_config();
        let account = make_account!(db);

        assert_matches!(
            delete_file(&db, get_root(&db).unwrap().id),
            Err(UiError(FileDeleteError::CannotDeleteRoot))
        );
    }

    #[test]
    fn test_create_parent_delete_parent_read_doc() {
        let db = test_config();
        let account = make_account!(db);
        let doc = create_file_at_path(&db, path!(account, "folder/test.md")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, doc.parent).unwrap();
        assert_matches!(
            read_document(&db, doc.id),
            Err(UiError(ReadDocumentError::FileDoesNotExist))
        );
    }

    #[test]
    fn test_create_parent_delete_parent_save_doc() {
        let db = test_config();
        let account = make_account!(db);
        let doc = create_file_at_path(&db, path!(account, "folder/test.md")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, doc.parent).unwrap();
        assert_matches!(
            save_document_to_disk(&db, doc.id, "/dev/null".to_string()),
            Err(UiError(SaveDocumentToDiskError::FileDoesNotExist))
        );
    }
}
