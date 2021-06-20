mod integration_test;

#[cfg(test)]
mod unit_tests {
    use crate::unit_tests::path_service::Filter::DocumentsOnly;
    use crate::unit_tests::path_service::Filter::FoldersOnly;
    use crate::unit_tests::path_service::Filter::LeafNodesOnly;
    use libsecp256k1::SecretKey;
    use lockbook_core::init_logger;
    use lockbook_core::model::state::temp_config;
    use lockbook_core::repo::{
        account_repo, document_repo, file_metadata_repo, local_changes_repo,
    };
    use lockbook_core::service::{
        file_encryption_service, file_service, integrity_service, path_service,
    };
    use lockbook_core::CoreError;
    use lockbook_models::account::Account;
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use rand::rngs::OsRng;
    use uuid::Uuid;
    macro_rules! assert_no_metadata_problems (
        ($db:expr) => {
            integrity_service::test_repo_integrity($db).unwrap()
        }
    );

    macro_rules! assert_total_local_changes (
        ($db:expr, $total:literal) => {
            assert_eq!(
                local_changes_repo::get_all_local_changes($db)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    macro_rules! assert_total_filtered_paths (
        ($db:expr, $filter:expr, $total:literal) => {
            assert_eq!(
                path_service::get_all_paths($db, $filter)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    fn test_account() -> Account {
        Account {
            username: String::from("username"),
            api_url: "ftp://uranus.net".to_string(),
            private_key: SecretKey::random(&mut OsRng),
        }
    }

    #[test]
    fn file_service_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        assert!(file_metadata_repo::get_root(config).unwrap().is_none());

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert!(file_metadata_repo::get_root(config).unwrap().is_some());
        assert_no_metadata_problems!(config);

        assert!(matches!(
            file_service::create(config, "", root.id, Document).unwrap_err(),
            CoreError::FileNameEmpty
        ));

        let folder1 = file_service::create(config, "TestFolder1", root.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder2 = file_service::create(config, "TestFolder2", folder1.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder3 = file_service::create(config, "TestFolder3", folder2.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder4 = file_service::create(config, "TestFolder4", folder3.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let folder5 = file_service::create(config, "TestFolder5", folder4.id, Folder).unwrap();
        assert_no_metadata_problems!(config);

        let file = file_service::create(config, "test.text", folder5.id, Document).unwrap();
        assert_no_metadata_problems!(config);

        assert_total_filtered_paths!(config, Some(FoldersOnly), 6);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 1);
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 1);

        file_service::write_document(config, file.id, "5 folders deep".as_bytes()).unwrap();

        assert_eq!(
            file_service::read_document(config, file.id).unwrap(),
            "5 folders deep".as_bytes()
        );
        assert!(file_service::read_document(config, folder4.id).is_err());
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn path_calculations_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        assert_total_filtered_paths!(config, None, 0);

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_total_filtered_paths!(config, None, 1);
        assert_eq!(
            path_service::get_all_paths(config, None)
                .unwrap()
                .get(0)
                .unwrap(),
            "username/"
        );

        assert_no_metadata_problems!(config);

        let folder1 = file_service::create(config, "TestFolder1", root.id, Folder).unwrap();
        assert_total_filtered_paths!(config, None, 2);
        assert!(path_service::get_all_paths(config, None)
            .unwrap()
            .contains(&"username/".to_string()));
        assert!(path_service::get_all_paths(config, None)
            .unwrap()
            .contains(&"username/TestFolder1/".to_string()));

        assert_no_metadata_problems!(config);

        let folder2 = file_service::create(config, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 = file_service::create(config, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 = file_service::create(config, "TestFolder4", folder3.id, Folder).unwrap();

        file_service::create(config, "TestFolder5", folder4.id, Folder).unwrap();
        file_service::create(config, "test1.text", folder4.id, Document).unwrap();
        file_service::create(config, "test2.text", folder2.id, Document).unwrap();
        file_service::create(config, "test3.text", folder2.id, Document).unwrap();
        file_service::create(config, "test4.text", folder2.id, Document).unwrap();
        file_service::create(config, "test5.text", folder2.id, Document).unwrap();
        assert_no_metadata_problems!(config);

        assert!(path_service::get_all_paths(config, None)
            .unwrap()
            .contains(&"username/TestFolder1/TestFolder2/test3.text".to_string()));
        assert!(path_service::get_all_paths(config, None).unwrap().contains(
            &"username/TestFolder1/TestFolder2/TestFolder3/TestFolder4/test1.text".to_string()
        ));
    }

    #[test]
    fn get_path_tests() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let folder1 = file_service::create(config, "TestFolder1", root.id, Folder).unwrap();
        let folder2 = file_service::create(config, "TestFolder2", folder1.id, Folder).unwrap();
        let folder3 = file_service::create(config, "TestFolder3", folder2.id, Folder).unwrap();
        let folder4 = file_service::create(config, "TestFolder4", folder3.id, Folder).unwrap();

        file_service::create(config, "TestFolder5", folder4.id, Folder).unwrap();
        file_service::create(config, "test1.text", folder4.id, Document).unwrap();
        file_service::create(config, "test2.text", folder2.id, Document).unwrap();
        let file = file_service::create(config, "test3.text", folder2.id, Document).unwrap();
        file_service::create(config, "test4.text", folder2.id, Document).unwrap();
        file_service::create(config, "test5.text", folder2.id, Document).unwrap();

        // match on this error more finely
        assert!(path_service::get_by_path(config, "invalid").is_err());
        assert!(
            path_service::get_by_path(config, "username/TestFolder1/TestFolder2/test3.text")
                .is_ok()
        );
        assert_eq!(
            path_service::get_by_path(config, "username/TestFolder1/TestFolder2/test3.text",)
                .unwrap(),
            file
        );

        path_service::get_all_paths(config, None)
            .unwrap()
            .into_iter()
            .for_each(|path| {
                path_service::get_by_path(config, &path).unwrap();
            });
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn test_arbitrary_path_file_creation() {
        init_logger(temp_config().path()).expect("Logger failed to initialize in test!");
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let paths_with_empties = ["username//", "username/path//to///file.md"];
        for path in &paths_with_empties {
            let err = path_service::create_at_path(config, path).unwrap_err();
            assert!(
                matches!(err, CoreError::PathContainsEmptyFileName),
                "Expected path \"{}\" to return PathContainsEmptyFile but instead it was {:?}",
                path,
                err
            );
        }

        assert!(path_service::create_at_path(config, "garbage").is_err());
        assert!(path_service::create_at_path(config, "username/").is_err());
        assert!(path_service::create_at_path(config, "username/").is_err());
        assert_total_filtered_paths!(config, None, 1);

        assert_eq!(
            file_encryption_service::get_name(
                config,
                &path_service::create_at_path(config, "username/test.txt").unwrap()
            )
            .unwrap(),
            "test.txt"
        );
        assert_total_filtered_paths!(config, None, 2);
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 1);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 1);
        assert_total_filtered_paths!(config, Some(FoldersOnly), 1);
        assert_no_metadata_problems!(config);

        assert_eq!(
            file_encryption_service::get_name(
                &config,
                &path_service::create_at_path(config, "username/folder1/folder2/folder3/test2.txt")
                    .unwrap()
            )
            .unwrap(),
            "test2.txt"
        );
        assert_total_filtered_paths!(config, None, 6);
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 2);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 2);
        assert_no_metadata_problems!(config);

        let file =
            path_service::create_at_path(config, "username/folder1/folder2/test3.txt").unwrap();
        assert_total_filtered_paths!(config, None, 7);
        assert_eq!(
            file_encryption_service::get_name(&config, &file).unwrap(),
            "test3.txt"
        );
        assert_eq!(
            file_encryption_service::get_name(
                &config,
                &file_metadata_repo::get(config, file.parent).unwrap()
            )
            .unwrap(),
            "folder2"
        );
        assert_eq!(
            file_metadata_repo::get(config, file.parent)
                .unwrap()
                .file_type,
            Folder
        );
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 3);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 3);

        assert_eq!(
            path_service::create_at_path(config, "username/folder1/folder2/folder3/folder4/")
                .unwrap()
                .file_type,
            Folder
        );
        assert_total_filtered_paths!(config, Some(DocumentsOnly), 3);
        assert_total_filtered_paths!(config, Some(LeafNodesOnly), 4);
        assert_total_filtered_paths!(config, Some(FoldersOnly), 5);
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_duplicate_files_via_path() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        path_service::create_at_path(config, "username/test.txt").unwrap();
        assert!(path_service::create_at_path(config, "username/test.txt").is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_duplicate_files_via_create() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let file = path_service::create_at_path(config, "username/test.txt").unwrap();
        assert!(file_service::create(config, "test.txt", file.parent, Document).is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_document_has_children_via_path() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        path_service::create_at_path(config, "username/test.txt").unwrap();
        assert!(path_service::create_at_path(config, "username/test.txt/oops.txt").is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_document_has_children() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let file = path_service::create_at_path(config, "username/test.txt").unwrap();
        assert!(file_service::create(config, "oops.txt", file.id, Document).is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn ensure_no_bad_names() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert!(file_service::create(config, "oops/txt", root.id, Document).is_err());

        assert_no_metadata_problems!(config);
    }

    #[test]
    fn rename_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_no_metadata_problems!(config);

        assert!(matches!(
            file_service::rename_file(config, root.id, "newroot").unwrap_err(),
            CoreError::RootModificationInvalid
        ));

        let file = path_service::create_at_path(config, "username/folder1/file1.txt").unwrap();
        assert!(
            local_changes_repo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .new
        );
        assert!(
            local_changes_repo::get_local_changes(config, file.parent)
                .unwrap()
                .unwrap()
                .new
        );
        assert_total_local_changes!(config, 2);
        assert_no_metadata_problems!(config);

        local_changes_repo::untrack_new_file(config, file.id).unwrap();
        local_changes_repo::untrack_new_file(config, file.parent).unwrap();
        assert_total_local_changes!(config, 0);

        file_service::rename_file(config, file.id, "file2.txt").unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .renamed
                .unwrap()
                .old_value,
            "file1.txt"
        );

        assert_no_metadata_problems!(config);

        file_service::rename_file(config, file.id, "file23.txt").unwrap();
        assert_total_local_changes!(config, 1);
        assert_eq!(
            local_changes_repo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .renamed
                .unwrap()
                .old_value,
            "file1.txt"
        );
        assert_total_local_changes!(config, 1);

        file_service::rename_file(config, file.id, "file1.txt").unwrap();
        assert_total_local_changes!(config, 0);
        assert_no_metadata_problems!(config);

        assert!(file_service::rename_file(config, Uuid::new_v4(), "not_used").is_err());
        assert!(file_service::rename_file(config, file.id, "file/1.txt").is_err());
        assert_total_local_changes!(config, 0);
        assert_eq!(
            file_encryption_service::get_name(
                &config,
                &file_metadata_repo::get(config, file.id).unwrap()
            )
            .unwrap(),
            "file1.txt"
        );

        let file2 = path_service::create_at_path(config, "username/folder1/file2.txt").unwrap();
        assert_eq!(
            file_encryption_service::get_name(
                &config,
                &file_metadata_repo::get(config, file2.id).unwrap()
            )
            .unwrap(),
            "file2.txt"
        );
        assert!(file_service::rename_file(config, file2.id, "file1.txt").is_err());
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn move_runthrough() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_no_metadata_problems!(config);

        assert!(matches!(
            file_service::move_file(config, root.id, Uuid::new_v4()).unwrap_err(),
            CoreError::RootModificationInvalid
        ));

        let file1 = path_service::create_at_path(config, "username/folder1/file.txt").unwrap();
        let og_folder = file1.parent;
        let folder1 = path_service::create_at_path(config, "username/folder2/").unwrap();
        assert!(
            file_service::write_document(config, folder1.id, &"should fail".as_bytes(),).is_err()
        );

        assert_no_metadata_problems!(config);

        file_service::write_document(config, file1.id, "nice doc ;)".as_bytes()).unwrap();

        assert_total_local_changes!(config, 3);
        assert_no_metadata_problems!(config);

        local_changes_repo::untrack_new_file(config, file1.id).unwrap();
        local_changes_repo::untrack_new_file(config, file1.parent).unwrap();
        local_changes_repo::untrack_new_file(config, folder1.id).unwrap();
        assert_total_local_changes!(config, 0);

        file_service::move_file(config, file1.id, folder1.id).unwrap();

        assert_eq!(
            file_service::read_document(config, file1.id).unwrap(),
            "nice doc ;)".as_bytes()
        );

        assert_no_metadata_problems!(config);

        assert_eq!(
            file_metadata_repo::get(config, file1.id).unwrap().parent,
            folder1.id
        );
        assert_total_local_changes!(config, 1);

        let file2 = path_service::create_at_path(config, "username/folder3/file.txt").unwrap();
        assert!(file_service::move_file(config, file1.id, file2.parent).is_err());
        assert!(file_service::move_file(config, Uuid::new_v4(), file2.parent).is_err());
        assert!(file_service::move_file(config, file1.id, Uuid::new_v4()).is_err());
        assert_total_local_changes!(config, 3);

        file_service::move_file(config, file1.id, og_folder).unwrap();
        assert_total_local_changes!(config, 2);
        assert_no_metadata_problems!(config);
    }

    #[test]
    fn test_move_folder_into_itself() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();
        assert_no_metadata_problems!(config);

        let folder1 = path_service::create_at_path(config, "username/folder1/").unwrap();
        let folder2 = path_service::create_at_path(config, "username/folder1/folder2/").unwrap();

        assert_total_local_changes!(config, 2);

        assert!(matches!(
            file_service::move_file(config, folder1.id, folder1.id).unwrap_err(),
            CoreError::FolderMovedIntoSelf
        ));

        assert!(matches!(
            file_service::move_file(config, folder1.id, folder2.id).unwrap_err(),
            CoreError::FolderMovedIntoSelf
        ));
    }

    #[test]
    fn test_keeping_track_of_edits() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();

        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let file = path_service::create_at_path(config, "username/file1.md").unwrap();
        file_service::write_document(config, file.id, "fresh content".as_bytes()).unwrap();

        assert!(
            local_changes_repo::get_local_changes(config, file.id)
                .unwrap()
                .unwrap()
                .new
        );

        local_changes_repo::untrack_new_file(config, file.id).unwrap();
        assert!(local_changes_repo::get_local_changes(config, file.id)
            .unwrap()
            .is_none());
        assert_total_local_changes!(config, 0);

        file_service::write_document(config, file.id, "fresh content2".as_bytes()).unwrap();
        assert!(local_changes_repo::get_local_changes(config, file.id)
            .unwrap()
            .unwrap()
            .content_edited
            .is_some());
        file_service::write_document(config, file.id, "fresh content".as_bytes()).unwrap();
        assert!(local_changes_repo::get_local_changes(config, file.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_document_delete_new_documents_no_trace_when_deleted() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let doc1 = file_service::create(config, "test1.md", root.id, Document).unwrap();

        file_service::write_document(config, doc1.id, &String::from("content").into_bytes())
            .unwrap();
        file_service::delete_document(config, doc1.id).unwrap();
        assert_total_local_changes!(config, 0);
        assert!(local_changes_repo::get_local_changes(config, doc1.id)
            .unwrap()
            .is_none());

        assert!(file_metadata_repo::maybe_get(config, doc1.id)
            .unwrap()
            .is_none());

        assert!(document_repo::maybe_get(config, doc1.id).unwrap().is_none());
    }

    #[test]
    fn test_document_delete_after_sync() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let doc1 = file_service::create(config, "test1.md", root.id, Document).unwrap();

        file_service::write_document(config, doc1.id, &String::from("content").into_bytes())
            .unwrap();
        local_changes_repo::delete(config, doc1.id).unwrap();

        file_service::delete_document(config, doc1.id).unwrap();
        assert_total_local_changes!(config, 1);
        assert!(
            local_changes_repo::get_local_changes(config, doc1.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        assert!(
            file_metadata_repo::maybe_get(config, doc1.id)
                .unwrap()
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn test_folders_are_created_in_order() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        path_service::create_at_path(config, &format!("{}/a/b/c/d/", account.username)).unwrap();
        let folder1 =
            path_service::get_by_path(config, &format!("{}/a/b/c/d/", account.username)).unwrap();
        let folder2 =
            path_service::get_by_path(config, &format!("{}/a/b/c/", account.username)).unwrap();
        let folder3 =
            path_service::get_by_path(config, &format!("{}/a/b/", account.username)).unwrap();
        let folder4 =
            path_service::get_by_path(config, &format!("{}/a/", account.username)).unwrap();

        assert_eq!(
            local_changes_repo::get_all_local_changes(config)
                .unwrap()
                .into_iter()
                .map(|change| change.id)
                .collect::<Vec<Uuid>>(),
            vec![folder4.id, folder3.id, folder2.id, folder1.id]
        );
    }

    #[test]
    fn test_delete_folder() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let folder1 = file_service::create(config, "folder1", root.id, Folder).unwrap();
        let document1 = file_service::create(config, "doc1", folder1.id, Document).unwrap();
        let document2 = file_service::create(config, "doc2", folder1.id, Document).unwrap();
        let document3 = file_service::create(config, "doc3", folder1.id, Document).unwrap();

        assert_total_local_changes!(config, 4);

        file_service::delete_folder(config, folder1.id).unwrap();
        assert_total_local_changes!(config, 1);

        assert!(file_metadata_repo::maybe_get(config, document1.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(config, document2.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(config, document3.id)
            .unwrap()
            .is_none());

        assert!(document_repo::maybe_get(config, document1.id)
            .unwrap()
            .is_none());
        assert!(document_repo::maybe_get(config, document2.id)
            .unwrap()
            .is_none());
        assert!(document_repo::maybe_get(config, document3.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_other_things_are_not_touched_during_delete() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        let folder1 = file_service::create(config, "folder1", root.id, Folder).unwrap();
        file_service::create(config, "doc1", folder1.id, Document).unwrap();
        file_service::create(config, "doc2", folder1.id, Document).unwrap();
        file_service::create(config, "doc3", folder1.id, Document).unwrap();

        let folder2 = file_service::create(config, "folder2", root.id, Folder).unwrap();
        let document4 = file_service::create(config, "doc1", folder2.id, Document).unwrap();
        let document5 = file_service::create(config, "doc2", folder2.id, Document).unwrap();
        let document6 = file_service::create(config, "doc3", folder2.id, Document).unwrap();

        assert_total_local_changes!(config, 8);

        file_service::delete_folder(config, folder1.id).unwrap();
        assert_total_local_changes!(config, 5);

        assert!(file_metadata_repo::maybe_get(config, document4.id)
            .unwrap()
            .is_some());
        assert!(file_metadata_repo::maybe_get(config, document5.id)
            .unwrap()
            .is_some());
        assert!(file_metadata_repo::maybe_get(config, document6.id)
            .unwrap()
            .is_some());

        assert!(document_repo::maybe_get(config, document4.id)
            .unwrap()
            .is_some());
        assert!(document_repo::maybe_get(config, document5.id)
            .unwrap()
            .is_some());
        assert!(document_repo::maybe_get(config, document6.id)
            .unwrap()
            .is_some());
    }

    #[test]
    fn test_cannot_delete_root() {
        let config = &temp_config();

        let account = test_account();
        account_repo::insert_account(config, &account).unwrap();
        let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
        file_metadata_repo::insert(config, &root).unwrap();

        assert!(matches!(
            file_service::delete_folder(config, root.id).unwrap_err(),
            CoreError::RootModificationInvalid
        ));

        assert_total_local_changes!(config, 0);
    }
}
