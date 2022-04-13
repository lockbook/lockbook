#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use lockbook_models::file_metadata::FileType;

    use crate::model::repo::RepoSource;
    use crate::pure_functions::files;
    use crate::repo::document_repo;
    use crate::service::test_utils::test_config;
    use crate::service::{file_service, test_utils};

    macro_rules! assert_metadata_changes_count (
        ($db:expr, $total:literal) => {
            assert_eq!(
                file_service::get_all_metadata_changes($db)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    macro_rules! assert_document_changes_count (
        ($db:expr, $total:literal) => {
            assert_eq!(
                file_service::get_all_with_document_changes($db)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    macro_rules! assert_metadata_nonexistent (
        ($db:expr, $source:expr, $id:expr) => {
            assert_eq!(
                file_service::maybe_get_metadata($db, $source, $id).unwrap(),
                None,
            );
        }
    );

    macro_rules! assert_metadata_eq (
        ($db:expr, $source:expr, $id:expr, $metadata:expr) => {
            assert_eq!(
                file_service::maybe_get_metadata($db, $source, $id).unwrap(),
                Some($metadata.clone()),
            );
        }
    );

    macro_rules! assert_document_eq (
        ($db:expr, $source:expr, $id:expr, $document:literal) => {
            assert_eq!(
                file_service::maybe_get_document($db, $source, $id).unwrap(),
                Some($document.to_vec()),
            );
        }
    );

    macro_rules! assert_metadata_count (
        ($db:expr, $source:expr, $total:literal) => {
            assert_eq!(
                file_service::get_all_metadata($db, $source)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    macro_rules! assert_document_count (
        ($db:expr, $source:expr, $total:literal) => {
            assert_eq!(
                file_service::get_all_metadata($db, $source)
                    .unwrap()
                    .iter()
                    .filter(|&f| document_repo::maybe_get($db, $source, f.id).unwrap().is_some() || document_repo::maybe_get($db, RepoSource::Base, f.id).unwrap().is_some())
                    .count(),
                $total
            );
        }
    );

    #[test]
    fn insert_metadata() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }
    #[test]
    fn merge_maybe_resolved_base() {
        let base = Some(0);
        let local = None;
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(0));
    }

    #[test]
    fn merge_maybe_resolved_local() {
        let base = None;
        let local = Some(1);
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(1));
    }

    #[test]
    fn merge_maybe_resolved_local_with_base() {
        let base = Some(0);
        let local = Some(1);
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(1));
    }

    #[test]
    fn merge_maybe_resolved_remote() {
        let base = None;
        let local = None;
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(2));
    }

    #[test]
    fn merge_maybe_resolved_remote_with_base() {
        let base = Some(0);
        let local = None;
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(2));
    }

    #[test]
    fn merge_maybe_resolved_conflict() {
        let base = Some(0);
        let local = Some(1);
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Conflict { base: 0, local: 1, remote: 2 });
    }

    #[test]
    fn merge_maybe_resolved_baseless_conflict() {
        let base = None;
        let local = Some(1);
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::BaselessConflict { local: 1, remote: 2 });
    }

    #[test]
    fn merge_maybe_none() {
        let base = None;
        let local = None;
        let remote = None;

        sync_service::merge_maybe::<i32>(base, local, remote).unwrap_err();
    }

    #[test]
    fn merge_metadata_local_and_remote_moved() {
        let account = &generate_account();
        let base = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("a33b99e8-140d-4a74-b564-f72efdcb5b3a").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Owner::from(account),
            decrypted_access_key: Default::default(),
        };
        let local = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c13f10f7-9360-4dd2-8b3a-0891a81c8bf8").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Owner::from(account),
            decrypted_access_key: Default::default(),
        };
        let remote = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786756,
            content_version: 1634693786556,
            deleted: false,
            owner: Owner::from(account),
            decrypted_access_key: Default::default(),
        };

        let result = sync_service::merge_metadata(base, local, remote);

        assert_eq!(
            result,
            DecryptedFileMetadata {
                id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
                file_type: FileType::Document,
                parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
                decrypted_name: String::from("test.txt"),
                metadata_version: 1634693786756,
                content_version: 1634693786556,
                deleted: false,
                owner: Owner::from(account),
                decrypted_access_key: Default::default(),
            }
        );
    }

    #[test]
    fn merge_maybe_metadata_local_and_remote_moved() {
        let account = &generate_account();
        let base = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("a33b99e8-140d-4a74-b564-f72efdcb5b3a").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Owner::from(account),
            decrypted_access_key: Default::default(),
        });
        let local = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c13f10f7-9360-4dd2-8b3a-0891a81c8bf8").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Owner::from(account),
            decrypted_access_key: Default::default(),
        });
        let remote = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786756,
            content_version: 1634693786556,
            deleted: false,
            owner: Owner::from(account),
            decrypted_access_key: Default::default(),
        });

        let result = sync_service::merge_maybe_metadata(base, local, remote).unwrap();

        assert_eq!(
            result,
            DecryptedFileMetadata {
                id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
                file_type: FileType::Document,
                parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
                decrypted_name: String::from("test.txt"),
                metadata_version: 1634693786756,
                content_version: 1634693786556,
                deleted: false,
                owner: Owner::from(account),
                decrypted_access_key: Default::default(),
            }
        );
    }

    #[test]
    fn get_metadata() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        let result = file_service::get_metadata(config, RepoSource::Local, root.id).unwrap();

        assert_eq!(result, root);
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn get_metadata_nonexistent() {
        let config = &test_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        let result = file_service::get_metadata(config, RepoSource::Local, Uuid::new_v4());

        assert!(result.is_err());
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 0);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn get_metadata_local_falls_back_to_base() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        let result = file_service::get_metadata(config, RepoSource::Local, root.id).unwrap();

        assert_eq!(result, root);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn get_metadata_local_prefers_local() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let mut root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();

        root.decrypted_name += " 2";

        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        let result = file_service::get_metadata(config, RepoSource::Local, root.id).unwrap();

        assert_eq!(result, root);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn maybe_get_metadata() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        let result = file_service::maybe_get_metadata(config, RepoSource::Local, root.id).unwrap();

        assert_eq!(result, Some(root));
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn maybe_get_metadata_nonexistent() {
        let config = &test_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        let result =
            file_service::maybe_get_metadata(config, RepoSource::Local, Uuid::new_v4()).unwrap();

        assert!(result.is_none());
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 0);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn insert_document() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();
        file_service::insert_document(config, RepoSource::Local, &document, b"document content")
            .unwrap();

        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn get_document() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();
        file_service::insert_document(config, RepoSource::Local, &document, b"document content")
            .unwrap();
        let result = file_service::get_document(config, RepoSource::Local, &document).unwrap();

        assert_eq!(result, b"document content");
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn get_document_nonexistent() {
        let config = &test_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        let result = file_service::get_document(
            config,
            RepoSource::Local,
            &files::create(
                FileType::Document,
                files::create_root(&account).id,
                "asdf",
                &account.public_key(),
            ),
        );

        assert!(result.is_err());
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 0);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn get_document_local_falls_back_to_base() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();
        let result = file_service::get_document(config, RepoSource::Local, &document).unwrap();

        assert_eq!(result, b"document content");
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn get_document_local_prefers_local() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();
        file_service::insert_document(config, RepoSource::Local, &document, b"document content 2")
            .unwrap();
        let result = file_service::get_document(config, RepoSource::Local, &document).unwrap();

        assert_eq!(result, b"document content 2");
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn maybe_get_document() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();
        let result =
            file_service::maybe_get_document(config, RepoSource::Local, &document).unwrap();

        assert_eq!(result, Some(b"document content".to_vec()));
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn maybe_get_document_nonexistent() {
        let config = &test_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        let result = file_service::maybe_get_document(
            config,
            RepoSource::Local,
            &files::create(
                FileType::Document,
                files::create_root(&account).id,
                "asdf",
                &account.public_key(),
            ),
        )
        .unwrap();

        assert!(result.is_none());
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 0);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn no_changes() {
        let config = &test_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 0);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn new() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert!(file_service::get_all_metadata_changes(config).unwrap()[0]
            .old_parent_and_name
            .is_none());
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn new_idempotent() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert!(file_service::get_all_metadata_changes(config).unwrap()[0]
            .old_parent_and_name
            .is_none());
        assert_metadata_count!(config, RepoSource::Base, 0);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn matching_base_and_local() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn matching_local_and_base() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn move_unmove() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let mut document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        document.parent = folder.id;
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert!(file_service::get_all_metadata_changes(config).unwrap()[0]
            .old_parent_and_name
            .is_some());
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        document.parent = root.id;
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn rename_unrename() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let mut document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        document.decrypted_name = String::from("document 2");
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert!(file_service::get_all_metadata_changes(config).unwrap()[0]
            .old_parent_and_name
            .is_some());
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        document.decrypted_name = String::from("document");
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn delete() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let mut document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        document.deleted = true;
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert!(file_service::get_all_metadata_changes(config).unwrap()[0]
            .old_parent_and_name
            .is_some());
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::prune_deleted(config).unwrap();
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn multiple_metadata_edits() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let mut root = files::create_root(&account);
        let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let mut document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        root.decrypted_name = String::from("root 2");
        folder.deleted = true;
        document.parent = folder.id;
        let document2 =
            files::create(FileType::Document, root.id, "document 2", &account.public_key());
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document2).unwrap();

        assert_metadata_changes_count!(config, 4);
        assert_document_changes_count!(config, 1);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 4);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn document_edit() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        file_service::insert_document(config, RepoSource::Local, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 1);
        assert_eq!(file_service::get_all_with_document_changes(config).unwrap()[0], document.id);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn document_edit_idempotent() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        file_service::insert_document(config, RepoSource::Local, &document, b"document content")
            .unwrap();
        file_service::insert_document(config, RepoSource::Local, &document, b"document content")
            .unwrap();
        file_service::insert_document(config, RepoSource::Local, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 1);
        assert_eq!(file_service::get_all_with_document_changes(config).unwrap()[0], document.id);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn document_edit_revert() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        file_service::insert_document(config, RepoSource::Local, &document, b"document content 2")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 1);
        assert_eq!(file_service::get_all_with_document_changes(config).unwrap()[0], document.id);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        file_service::insert_document(config, RepoSource::Local, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn document_edit_manual_promote() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        file_service::insert_document(config, RepoSource::Local, &document, b"document content 2")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 1);
        assert_eq!(file_service::get_all_with_document_changes(config).unwrap()[0], document.id);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        file_service::insert_document(config, RepoSource::Base, &document, b"document content 2")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn promote() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let mut root = files::create_root(&account);
        let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let mut document =
            files::create(FileType::Document, folder.id, "document", &account.public_key());
        let document2 =
            files::create(FileType::Document, folder.id, "document 2", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document2).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();
        file_service::insert_document(config, RepoSource::Base, &document2, b"document 2 content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 4);
        assert_metadata_count!(config, RepoSource::Local, 4);
        assert_document_count!(config, RepoSource::Base, 2);
        assert_document_count!(config, RepoSource::Local, 2);

        root.decrypted_name = String::from("root 2");
        folder.deleted = true;
        document.parent = root.id;
        let document3 =
            files::create(FileType::Document, root.id, "document 3", &account.public_key());
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document3).unwrap();
        file_service::insert_document(config, RepoSource::Local, &document, b"document content 2")
            .unwrap();
        file_service::insert_document(config, RepoSource::Local, &document3, b"document 3 content")
            .unwrap();

        assert_metadata_changes_count!(config, 4);
        assert_document_changes_count!(config, 2);
        assert_metadata_count!(config, RepoSource::Base, 4);
        assert_metadata_count!(config, RepoSource::Local, 5);
        assert_document_count!(config, RepoSource::Base, 2);
        assert_document_count!(config, RepoSource::Local, 3);

        file_service::promote_metadata(config).unwrap();
        file_service::promote_documents(config).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_eq!(config, RepoSource::Base, root.id, root);
        assert_metadata_eq!(config, RepoSource::Base, folder.id, folder);
        assert_metadata_eq!(config, RepoSource::Base, document.id, document);
        assert_metadata_eq!(config, RepoSource::Base, document2.id, document2);
        assert_metadata_eq!(config, RepoSource::Base, document3.id, document3);
        assert_document_eq!(config, RepoSource::Base, &document, b"document content 2");
        assert_document_eq!(config, RepoSource::Base, &document2, b"document 2 content");
        assert_document_eq!(config, RepoSource::Base, &document3, b"document 3 content");
        assert_metadata_count!(config, RepoSource::Base, 5);
        assert_metadata_count!(config, RepoSource::Local, 5);
        assert_document_count!(config, RepoSource::Base, 3);
        assert_document_count!(config, RepoSource::Local, 3);
    }

    #[test]
    fn prune_deleted() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let mut document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        document.deleted = true;
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_nonexistent!(config, RepoSource::Base, document.id);
        assert_metadata_nonexistent!(config, RepoSource::Local, document.id);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn prune_deleted_document_edit() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let mut document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        document.deleted = true;
        file_service::insert_document(config, RepoSource::Local, &document, b"document content 2")
            .unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_nonexistent!(config, RepoSource::Base, document.id);
        assert_metadata_nonexistent!(config, RepoSource::Local, document.id);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn prune_deleted_document_in_deleted_folder() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, folder.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        folder.deleted = true;
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_nonexistent!(config, RepoSource::Base, folder.id);
        assert_metadata_nonexistent!(config, RepoSource::Local, folder.id);
        assert_metadata_nonexistent!(config, RepoSource::Base, document.id);
        assert_metadata_nonexistent!(config, RepoSource::Local, document.id);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn prune_deleted_document_moved_from_deleted_folder() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let mut document =
            files::create(FileType::Document, folder.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        folder.deleted = true;
        document.parent = root.id;
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_nonexistent!(config, RepoSource::Base, folder.id);
        assert_metadata_nonexistent!(config, RepoSource::Local, folder.id);
        assert_metadata_eq!(config, RepoSource::Base, document.id, document);
        assert_metadata_eq!(config, RepoSource::Local, document.id, document);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn prune_deleted_base_only() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let mut document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        let mut document_local = document.clone();
        document_local.decrypted_name = String::from("renamed document");
        file_service::insert_metadatum(config, RepoSource::Local, &document_local).unwrap();
        document.deleted = true;
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert_metadata_eq!(config, RepoSource::Base, document.id, document);
        assert_metadata_eq!(config, RepoSource::Local, document.id, document_local);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn prune_deleted_local_only() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        let mut document_deleted = document.clone();
        document_deleted.deleted = true;
        file_service::insert_metadatum(config, RepoSource::Local, &document_deleted).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert_metadata_eq!(config, RepoSource::Base, document.id, document);
        assert_metadata_eq!(config, RepoSource::Local, document.id, document_deleted);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn prune_deleted_document_moved_from_deleted_folder_local_only() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, folder.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        let mut folder_deleted = folder;
        folder_deleted.deleted = true;
        let mut document_moved = document.clone();
        document_moved.parent = root.id;
        file_service::insert_metadatum(config, RepoSource::Base, &folder_deleted).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &folder_deleted).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document_moved).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 1);
        assert_document_changes_count!(config, 0);
        assert_metadata_eq!(config, RepoSource::Base, document.id, document);
        assert_metadata_eq!(config, RepoSource::Local, document.id, document_moved);
        assert_metadata_count!(config, RepoSource::Base, 3);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn prune_deleted_new_local_deleted_folder() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);

        let mut deleted_folder =
            files::create(FileType::Folder, root.id, "folder", &account.public_key());
        deleted_folder.deleted = true;
        file_service::insert_metadatum(config, RepoSource::Local, &deleted_folder).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 1);
        assert_metadata_count!(config, RepoSource::Local, 1);
        assert_document_count!(config, RepoSource::Base, 0);
        assert_document_count!(config, RepoSource::Local, 0);
    }

    #[test]
    fn prune_deleted_new_local_deleted_folder_with_existing_moved_child() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        let mut deleted_folder =
            files::create(FileType::Folder, root.id, "folder", &account.public_key());
        deleted_folder.deleted = true;
        let mut document_moved = document.clone();
        document_moved.parent = deleted_folder.id;
        file_service::insert_metadatum(config, RepoSource::Local, &deleted_folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document_moved).unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 2);
        assert_document_changes_count!(config, 0);
        assert_metadata_eq!(config, RepoSource::Local, document.id, document_moved);
        assert_metadata_eq!(config, RepoSource::Local, deleted_folder.id, deleted_folder);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }

    #[test]
    fn prune_deleted_new_local_deleted_folder_with_deleted_existing_moved_child() {
        let config = &test_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"document content")
            .unwrap();

        assert_metadata_changes_count!(config, 0);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 2);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);

        let mut deleted_folder =
            files::create(FileType::Folder, root.id, "folder", &account.public_key());
        deleted_folder.deleted = true;
        let mut document_moved_and_deleted = document;
        document_moved_and_deleted.parent = deleted_folder.id;
        document_moved_and_deleted.deleted = true;
        file_service::insert_metadatum(config, RepoSource::Local, &deleted_folder).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &document_moved_and_deleted)
            .unwrap();
        file_service::prune_deleted(config).unwrap();

        assert_metadata_changes_count!(config, 2);
        assert_document_changes_count!(config, 0);
        assert_metadata_count!(config, RepoSource::Base, 2);
        assert_metadata_count!(config, RepoSource::Local, 3);
        assert_document_count!(config, RepoSource::Base, 1);
        assert_document_count!(config, RepoSource::Local, 1);
    }
}
