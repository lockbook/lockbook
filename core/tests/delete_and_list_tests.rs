mod integration_test;

#[cfg(test)]
mod delete_and_list_tests {
    use lockbook_core::service::path_service::Filter;
    use lockbook_core::service::test_utils::create_account;
    use lockbook_core::service::test_utils::test_config;
    use lockbook_core::Error::UiError;
    use lockbook_core::{
        assert_matches, create_file, create_file_at_path, delete_file, get_root, list_metadatas,
        list_paths, move_file, path, read_document, rename_file, save_document_to_disk,
        write_document, CreateFileError, FileDeleteError, MoveFileError, ReadDocumentError,
        RenameFileError, SaveDocumentToDiskError, WriteToDocumentError,
    };
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn test_create_delete_list() {
        let db = test_config();
        let account = create_account(&db);
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
        let account = create_account(&db);
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
        let account = create_account(&db);
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
        let account = create_account(&db);
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
        let _account = create_account(&db);

        assert_matches!(
            delete_file(&db, get_root(&db).unwrap().id),
            Err(UiError(FileDeleteError::CannotDeleteRoot))
        );
    }

    #[test]
    fn test_create_parent_delete_parent_read_doc() {
        let db = test_config();
        let account = create_account(&db);
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
        let account = create_account(&db);
        let doc = create_file_at_path(&db, path!(account, "folder/test.md")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, doc.parent).unwrap();
        assert_matches!(
            save_document_to_disk(&db, doc.id, "/dev/null"),
            Err(UiError(SaveDocumentToDiskError::FileDoesNotExist))
        );
    }

    #[test]
    fn test_create_parent_delete_parent_rename_doc() {
        let db = test_config();
        let account = create_account(&db);
        let doc = create_file_at_path(&db, path!(account, "folder/test.md")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, doc.parent).unwrap();
        assert_matches!(
            rename_file(&db, doc.id, "test2.md"),
            Err(UiError(RenameFileError::FileDoesNotExist))
        );
    }

    #[test]
    fn test_create_parent_delete_parent_rename_parent() {
        let db = test_config();
        let account = create_account(&db);
        let doc = create_file_at_path(&db, path!(account, "folder/test.md")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, doc.parent).unwrap();
        assert_matches!(
            rename_file(&db, doc.parent, "folder2"),
            Err(UiError(RenameFileError::FileDoesNotExist))
        );
    }

    #[test]
    fn test_folder_move_delete_source_parent() {
        let db = test_config();
        let account = create_account(&db);
        let doc = create_file_at_path(&db, path!(account, "folder1/test.md")).unwrap();
        let folder2 = create_file_at_path(&db, path!(account, "folder2")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, doc.parent).unwrap();
        assert_matches!(
            move_file(&db, doc.id, folder2.id),
            Err(UiError(MoveFileError::FileDoesNotExist))
        );
    }

    #[test]
    fn test_folder_move_delete_source_doc() {
        let db = test_config();
        let account = create_account(&db);
        let doc = create_file_at_path(&db, path!(account, "folder1/test.md")).unwrap();
        let folder2 = create_file_at_path(&db, path!(account, "folder2")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, doc.parent).unwrap();
        assert_matches!(
            move_file(&db, doc.parent, folder2.id),
            Err(UiError(MoveFileError::FileDoesNotExist))
        );
    }

    #[test]
    fn test_folder_move_delete_destination_parent() {
        let db = test_config();
        let account = create_account(&db);
        let doc = create_file_at_path(&db, path!(account, "folder1/test.md")).unwrap();
        let folder2 = create_file_at_path(&db, path!(account, "folder2")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, folder2.id).unwrap();
        assert_matches!(
            move_file(&db, doc.id, folder2.id),
            Err(UiError(MoveFileError::TargetParentDoesNotExist))
        );
    }

    #[test]
    fn test_folder_move_delete_destination_doc() {
        let db = test_config();
        let account = create_account(&db);
        let doc = create_file_at_path(&db, path!(account, "folder1/test.md")).unwrap();
        let folder2 = create_file_at_path(&db, path!(account, "folder2")).unwrap();
        write_document(&db, doc.id, "content".as_bytes()).unwrap();
        assert_eq!(read_document(&db, doc.id).unwrap(), "content".as_bytes());
        delete_file(&db, folder2.id).unwrap();
        assert_matches!(
            move_file(&db, doc.parent, folder2.id),
            Err(UiError(MoveFileError::TargetParentDoesNotExist))
        );
    }

    #[test]
    fn test_delete_list_files() {
        let db = test_config();
        let account = create_account(&db);
        let f1 = create_file_at_path(&db, path!(account, "f1/")).unwrap();
        let _f2 = create_file_at_path(&db, path!(account, "f1/f2/")).unwrap();
        let d1 = create_file_at_path(&db, path!(account, "f1/f2/d1.md")).unwrap();
        delete_file(&db, f1.id).unwrap();

        let mut files = list_metadatas(&db).unwrap();
        files.retain(|meta| meta.id == d1.id);

        assert!(files.is_empty());
    }
}
