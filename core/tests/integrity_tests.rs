#[cfg(test)]
mod integrity_tests {
    use lockbook_core::service::test_utils;
    use rand::Rng;

    use lockbook_core::model::repo::RepoSource;
    use lockbook_core::repo::metadata_repo;
    use lockbook_core::service::file_service;
    use lockbook_core::service::integrity_service;
    use lockbook_core::service::integrity_service::TestRepoError::*;
    use lockbook_core::service::integrity_service::Warning;
    use lockbook_core::{assert_matches, create_file_at_path, get_file_by_path};
    use lockbook_models::file_metadata::FileType::Document;

    #[test]
    fn test_integrity_no_problems() {
        let cfg = test_utils::test_config();
        let (_account, _root) = test_utils::create_account(&cfg);
        integrity_service::test_repo_integrity(&cfg).unwrap();
    }

    #[test]
    fn test_integrity_no_problems_but_more_complicated() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        create_file_at_path(&cfg, &test_utils::path(&root, "/doc.md")).unwrap();
        integrity_service::test_repo_integrity(&cfg).unwrap();
    }

    #[test]
    fn test_ok() {
        let cfg = test_utils::test_config();
        let (_account, _root) = test_utils::create_account(&cfg);

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Ok(_));
    }

    #[test]
    fn test_no_account() {
        let cfg = test_utils::test_config();

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(NoAccount));
    }

    #[test]
    fn test_no_root() {
        let cfg = test_utils::test_config();
        let (_account, _root) = test_utils::create_account(&cfg);
        metadata_repo::delete_all(&cfg, RepoSource::Base).unwrap();

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(NoRootFolder));
    }

    #[test]
    fn test_orphaned_children() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/document1.md"))
            .unwrap();

        integrity_service::test_repo_integrity(&cfg).unwrap();

        metadata_repo::delete(
            &cfg,
            RepoSource::Local,
            get_file_by_path(&cfg, &test_utils::path(&root, "/folder1"))
                .unwrap()
                .id,
        )
        .unwrap();

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(FileOrphaned(_)));
    }

    #[test]
    fn test_invalid_file_name_slash() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        let doc = create_file_at_path(&cfg, &test_utils::path(&root, "/document1.md")).unwrap();
        let mut doc = file_service::get_metadata(&cfg, RepoSource::Local, doc.id).unwrap();
        doc.decrypted_name = String::from("na/me.md");
        file_service::insert_metadatum(&cfg, RepoSource::Local, &doc).unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(FileNameContainsSlash(_))
        );
    }

    #[test]
    fn test_invalid_file_name_empty() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        let doc = create_file_at_path(&cfg, &test_utils::path(&root, "/document1.md")).unwrap();
        let mut doc = file_service::get_metadata(&cfg, RepoSource::Local, doc.id).unwrap();
        doc.decrypted_name = String::from("");
        file_service::insert_metadatum(&cfg, RepoSource::Local, &doc).unwrap();

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(FileNameEmpty(_)));
    }

    #[test]
    fn test_cycle() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/document1.md"))
            .unwrap();
        let mut parent = metadata_repo::get(
            &cfg,
            RepoSource::Local,
            get_file_by_path(&cfg, &test_utils::path(&root, "/folder1"))
                .unwrap()
                .id,
        )
        .unwrap();
        let child = get_file_by_path(&cfg, &test_utils::path(&root, "/folder1/folder2")).unwrap();
        parent.parent = child.id;
        metadata_repo::insert(&cfg, RepoSource::Local, &parent).unwrap();

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(CycleDetected(_)));
    }

    #[test]
    fn test_cycle_with_three_files() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);

        let _folder1 = create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/")).unwrap();
        let _folder2 =
            create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/")).unwrap();
        let folder3 =
            create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/folder3/"))
                .unwrap();

        let mut parent = metadata_repo::get(
            &cfg,
            RepoSource::Local,
            get_file_by_path(&cfg, &test_utils::path(&root, "/folder1"))
                .unwrap()
                .id,
        )
        .unwrap();
        parent.parent = folder3.id;
        metadata_repo::insert(&cfg, RepoSource::Local, &parent).unwrap();

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(CycleDetected(_)));
    }

    #[test]
    fn test_documents_treated_as_folders() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/document1.md"))
            .unwrap();
        let mut parent = metadata_repo::get(
            &cfg,
            RepoSource::Local,
            get_file_by_path(&cfg, &test_utils::path(&root, "/folder1"))
                .unwrap()
                .id,
        )
        .unwrap();
        parent.file_type = Document;
        metadata_repo::insert(&cfg, RepoSource::Local, &parent).unwrap();

        assert_matches!(
            integrity_service::test_repo_integrity(&cfg),
            Err(DocumentTreatedAsFolder(_))
        );
    }

    #[test]
    fn test_name_conflict() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        let doc = create_file_at_path(&cfg, &test_utils::path(&root, "/document1.md")).unwrap();
        create_file_at_path(&cfg, &test_utils::path(&root, "/document2.md")).unwrap();
        let mut doc = file_service::get_metadata(&cfg, RepoSource::Local, doc.id).unwrap();
        doc.decrypted_name = String::from("document2.md");
        file_service::insert_metadatum(&cfg, RepoSource::Local, &doc).unwrap();

        assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(NameConflictDetected(_)));
    }

    #[test]
    fn test_empty_file() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        let doc = create_file_at_path(&cfg, &test_utils::path(&root, "/document.txt")).unwrap();
        let doc = file_service::get_metadata(&cfg, RepoSource::Local, doc.id).unwrap();
        file_service::insert_document(&cfg, RepoSource::Local, &doc, "".as_bytes()).unwrap();

        let warnings = integrity_service::test_repo_integrity(&cfg);

        assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([Warning::EmptyFile(_)]));
    }

    #[test]
    fn test_invalid_utf8() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        let doc = create_file_at_path(&cfg, &test_utils::path(&root, "/document.txt")).unwrap();
        let doc = file_service::get_metadata(&cfg, RepoSource::Local, doc.id).unwrap();
        file_service::insert_document(
            &cfg,
            RepoSource::Local,
            &doc,
            rand::thread_rng().gen::<[u8; 32]>().as_ref(),
        )
        .unwrap();

        let warnings = integrity_service::test_repo_integrity(&cfg);

        assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([Warning::InvalidUTF8(_)]));
    }

    #[test]
    fn test_invalid_utf8_ignores_non_utf_file_extensions() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        let doc = create_file_at_path(&cfg, &test_utils::path(&root, "/document.png")).unwrap();
        let doc = file_service::get_metadata(&cfg, RepoSource::Local, doc.id).unwrap();
        file_service::insert_document(
            &cfg,
            RepoSource::Local,
            &doc,
            rand::thread_rng().gen::<[u8; 32]>().as_ref(),
        )
        .unwrap();

        let warnings = integrity_service::test_repo_integrity(&cfg);

        assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([]));
    }

    #[test]
    fn test_invalid_drawing() {
        let cfg = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&cfg);
        let doc = create_file_at_path(&cfg, &test_utils::path(&root, "/document.draw")).unwrap();
        let doc = file_service::get_metadata(&cfg, RepoSource::Local, doc.id).unwrap();
        file_service::insert_document(
            &cfg,
            RepoSource::Local,
            &doc,
            rand::thread_rng().gen::<[u8; 32]>().as_ref(),
        )
        .unwrap();

        let warnings = integrity_service::test_repo_integrity(&cfg);

        assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([Warning::UnreadableDrawing(_)]));
    }
}
