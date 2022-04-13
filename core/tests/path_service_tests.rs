
#[cfg(test)]
mod unit_tests {
    use lockbook_models::file_metadata::FileType;

    use crate::model::repo::RepoSource;
    use crate::model::state::temp_config;
    use crate::pure_functions::files;
    use crate::repo::account_repo;
    use crate::service::path_service::Filter;
    use crate::service::{file_service, path_service, test_utils};
    use crate::CoreError;

    #[test]
    fn create_at_path_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let doc = path_service::create_at_path(config, &format!("{}/document", &account.username))
            .unwrap();

        assert_eq!(doc.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();

        assert_eq!(folder.file_type, FileType::Folder);
    }

    #[test]
    fn create_at_path_in_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();
        let document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();

        assert_eq!(folder.file_type, FileType::Folder);
        assert_eq!(document.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_missing_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();
        let folder =
            path_service::get_by_path(config, &format!("{}/folder", &account.username)).unwrap();

        assert_eq!(folder.file_type, FileType::Folder);
        assert_eq!(document.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_missing_folders() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document = path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
            .unwrap();
        let folder1 =
            path_service::get_by_path(config, &format!("{}/folder", &account.username)).unwrap();
        let folder2 =
            path_service::get_by_path(config, &format!("{}/folder/folder", &account.username))
                .unwrap();

        assert_eq!(folder1.file_type, FileType::Folder);
        assert_eq!(folder2.file_type, FileType::Folder);
        assert_eq!(document.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_path_contains_empty_file_name() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let result =
            path_service::create_at_path(config, &format!("{}//document", &account.username));

        assert_eq!(result, Err(CoreError::PathContainsEmptyFileName));
    }

    #[test]
    fn create_at_path_path_starts_with_non_root() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let result = path_service::create_at_path(
            config,
            &format!("{}/folder/document", "not-account-username"),
        );

        assert_eq!(result, Err(CoreError::PathStartsWithNonRoot));
    }

    #[test]
    fn create_at_path_path_taken() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
            .unwrap();
        let result =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username));

        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn create_at_path_not_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(config, &format!("{}/not-folder", &account.username)).unwrap();
        let result = path_service::create_at_path(
            config,
            &format!("{}/not-folder/document", &account.username),
        );

        assert_eq!(result, Err(CoreError::FileNotFolder));
    }

    #[test]
    fn get_by_path_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let created_document =
            path_service::create_at_path(config, &format!("{}/document", &account.username))
                .unwrap();
        let document =
            path_service::get_by_path(config, &format!("{}/document", &account.username)).unwrap();

        assert_eq!(created_document, document);
    }

    #[test]
    fn get_by_path_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let created_folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();
        let folder =
            path_service::get_by_path(config, &format!("{}/folder", &account.username)).unwrap();

        assert_eq!(created_folder, folder);
    }

    #[test]
    fn get_by_path_document_in_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let created_document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();
        let document =
            path_service::get_by_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();

        assert_eq!(created_document, document);
    }

    #[test]
    fn get_path_by_id_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document =
            path_service::create_at_path(config, &format!("{}/document", &account.username))
                .unwrap();
        let document_path = path_service::get_path_by_id(config, document.id).unwrap();

        assert_eq!(&document_path, &format!("{}/document", &account.username));
    }

    #[test]
    fn get_path_by_id_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();
        let folder_path = path_service::get_path_by_id(config, folder.id).unwrap();

        assert_eq!(&folder_path, &format!("{}/folder/", &account.username));
    }

    #[test]
    fn get_path_by_id_document_in_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();
        let document_path = path_service::get_path_by_id(config, document.id).unwrap();

        assert_eq!(&document_path, &format!("{}/folder/document", &account.username));
    }

    #[test]
    fn get_all_paths() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
            .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
            .unwrap();

        let all_paths = path_service::get_all_paths(config, None).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
        assert_eq!(all_paths.len(), 5);
    }

    #[test]
    fn get_all_paths_documents_only() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
            .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
            .unwrap();

        let all_paths = path_service::get_all_paths(config, Some(Filter::DocumentsOnly)).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
        assert_eq!(all_paths.len(), 1);
    }

    #[test]
    fn get_all_paths_folders_only() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
            .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
            .unwrap();

        let all_paths = path_service::get_all_paths(config, Some(Filter::FoldersOnly)).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
        assert_eq!(all_paths.len(), 4);
    }

    #[test]
    fn get_all_paths_leaf_nodes_only() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
            .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
            .unwrap();

        let all_paths = path_service::get_all_paths(config, Some(Filter::LeafNodesOnly)).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
        assert_eq!(all_paths.len(), 2);
    }
}