#[cfg(test)]
mod integrity_tests {
    use lockbook_core::repo::file_metadata_repo;
    use lockbook_core::service::integrity_service::TestRepoError::*;
    use lockbook_core::service::test_utils::*;
    use lockbook_core::service::{file_encryption_service, integrity_service};
    use lockbook_core::{assert_matches, get_file_by_path, path};
    use lockbook_core::{create_account, create_file_at_path};
    use lockbook_models::file_metadata::FileType::Document;

    #[test]
    fn test_integrity_no_problems() {
        let cfg = test_config();
        create_account(&cfg, &random_username(), &url()).unwrap();
        integrity_service::test_repo_integrity(&cfg).unwrap();
    }

    #[test]
    fn test_no_root() {
        let cfg = test_config();
        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(NoRootFolder)
        );
    }

    #[test]
    fn test_orphaned_children() {
        let cfg = test_config();
        let account = create_account(&cfg, &random_username(), &url()).unwrap();
        create_file_at_path(&cfg, path!(account, "folder1/folder2/document1.md")).unwrap();

        integrity_service::test_repo_integrity(&cfg).unwrap();

        file_metadata_repo::non_recursive_delete(
            &cfg,
            get_file_by_path(&cfg, path!(account, "folder1"))
                .unwrap()
                .id,
        )
        .unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(FileOrphaned(_))
        );
    }

    #[test]
    fn test_invalid_file_name_slash() {
        let cfg = test_config();
        let account = create_account(&cfg, &random_username(), &url()).unwrap();
        let doc = create_file_at_path(&cfg, path!(account, "document1.md")).unwrap();
        let mut doc = file_metadata_repo::get(&cfg, doc.id).unwrap();
        doc.name = file_encryption_service::create_name(&cfg, &doc, "na/me.md").unwrap();
        file_metadata_repo::insert(&cfg, &doc).unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(FileNameContainsSlash(_))
        );
    }

    #[test]
    fn test_invalid_file_name_empty() {
        let cfg = test_config();
        let account = create_account(&cfg, &random_username(), &url()).unwrap();
        let doc = create_file_at_path(&cfg, path!(account, "document1.md")).unwrap();
        let mut doc = file_metadata_repo::get(&cfg, doc.id).unwrap();
        doc.name = file_encryption_service::create_name(&cfg, &doc, "").unwrap();
        file_metadata_repo::insert(&cfg, &doc).unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(FileNameEmpty(_))
        );
    }

    #[test]
    fn test_cycle() {
        let cfg = test_config();
        let account = create_account(&cfg, &random_username(), &url()).unwrap();
        create_file_at_path(&cfg, path!(account, "folder1/folder2/document1.md")).unwrap();
        let mut parent = file_metadata_repo::get(
            &cfg,
            get_file_by_path(&cfg, path!(account, "folder1"))
                .unwrap()
                .id,
        )
        .unwrap();
        let child = get_file_by_path(&cfg, path!(account, "folder1/folder2")).unwrap();
        parent.parent = child.id;
        file_metadata_repo::insert(&cfg, &parent).unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(CycleDetected(_))
        );
    }

    #[test]
    fn test_documents_treated_as_folders() {
        let cfg = test_config();
        let account = create_account(&cfg, &random_username(), &url()).unwrap();
        create_file_at_path(&cfg, path!(account, "folder1/folder2/document1.md")).unwrap();
        let mut parent = file_metadata_repo::get(
            &cfg,
            get_file_by_path(&cfg, path!(account, "folder1"))
                .unwrap()
                .id,
        )
        .unwrap();
        parent.file_type = Document;
        file_metadata_repo::insert(&cfg, &parent).unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(DocumentTreatedAsFolder(_))
        );
    }

    #[test]
    fn test_name_conflict() {
        let cfg = test_config();
        let account = create_account(&cfg, &random_username(), &url()).unwrap();
        let doc = create_file_at_path(&cfg, path!(account, "document1.md")).unwrap();
        create_file_at_path(&cfg, path!(account, "document2.md")).unwrap();
        let mut doc = file_metadata_repo::get(&cfg, doc.id).unwrap();
        doc.name = file_encryption_service::create_name(&cfg, &doc, "document2.md").unwrap();
        file_metadata_repo::insert(&cfg, &doc).unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(NameConflictDetected(_))
        );
    }
}
