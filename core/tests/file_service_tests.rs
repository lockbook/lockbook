use hmdb::transaction::Transaction;
use lockbook_core::model::repo::RepoSource;
use lockbook_core::pure_functions::files;
use lockbook_core::repo::document_repo;
use lockbook_core::service::sync_service::MaybeMergeResult;
use lockbook_core::service::{file_service, sync_service};
use lockbook_models::file_metadata::Owner;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
use lockbook_models::tree::FileMetadata;
use std::str::FromStr;
use test_utils::*;
use uuid::Uuid;

macro_rules! assert_metadata_changes_count (
    ($core:expr, $total:literal) => {
        assert_eq!(
            $core.db.transaction(|tx| {
                tx.get_all_metadata_changes()
                .unwrap()
                .len()
            }).unwrap(),
            $total
        );
    }
);

macro_rules! assert_document_changes_count (
    ($core:expr, $total:literal) => {
        assert_eq!(
            $core.db.transaction(|tx| {
                tx.get_all_with_document_changes(&$core.config)
                .unwrap()
                .len()
            }).unwrap(),
            $total
        );
    }
);

macro_rules! assert_metadata_nonexistent (
    ($core:expr, $source:expr, $id:expr) => {
        assert!(
            $core.db
                .transaction(|tx| tx.maybe_get_metadata($source, $id))
                .unwrap()
                .unwrap()
                .is_none()
        );
    }
);

macro_rules! assert_metadata_eq (
    ($core:expr, $source:expr, $id:expr, $metadata:expr) => {
        assert_eq!(
            $core.db.transaction(|tx| tx.maybe_get_metadata($source, $id)).unwrap().unwrap(),
            Some($metadata.clone()),
        );
    }
);

macro_rules! assert_document_eq (
($core:expr, $source:expr, $id:expr, $document:literal) => {
    assert_eq!(
        file_service::maybe_get_document(&$core.config, $source, $id).unwrap(),
            Some($document.to_vec()),
        );
    }
);

macro_rules! assert_metadata_count (
($core:expr, $source:expr, $total:literal) => {
    assert_eq!(
        $core.db.transaction(|tx| tx.get_all_metadata($source).unwrap().len()).unwrap(),
        $total
    );
});

macro_rules! assert_document_count (
($core:expr, $source:expr, $total:literal) => {
    assert_eq!(
        $core.db.transaction(|tx| tx.get_all_metadata($source).unwrap())
            .unwrap()
            .iter()
            .filter(
                |&f| document_repo::maybe_get(&$core.config, $source, f.id).unwrap().is_some()
                    || document_repo::maybe_get(&$core.config, RepoSource::Base, f.id).unwrap().is_some()
                )
            .count(),
            $total
        );
    }
);

#[test]
fn insert_metadata() {
    let core = test_core_with_account();

    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
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
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();

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
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
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
    let core = test_core_with_account();

    core.db
        .transaction(|tx| {
            let root = tx.root().unwrap();
            tx.get_metadata(RepoSource::Local, root.id).unwrap()
        })
        .unwrap();
}

#[test]
fn get_metadata_nonexistent() {
    let core = test_core_with_account();

    core.db
        .transaction(|tx| assert!(tx.get_metadata(RepoSource::Local, Uuid::new_v4()).is_err()))
        .unwrap();
}

#[test]
fn get_metadata_local_falls_back_to_base() {
    let core = test_core_with_account();
    let dir = core.create_at_path(&path(&core, "test/")).unwrap();
    let db_dir = core.db.local_metadata.delete(dir.id).unwrap().unwrap();
    core.db.base_metadata.insert(db_dir.id, db_dir).unwrap();

    let result = core
        .db
        .transaction(|tx| tx.get_metadata(RepoSource::Local, dir.id).unwrap())
        .unwrap();

    assert_eq!(result, dir);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn get_metadata_local_prefers_local() {
    let core = test_core_with_account();

    let initial_doc = core
        .db
        .transaction(|tx| {
            let root = tx.root().unwrap().id;
            let initial_doc = tx
                .create_file(&core.config, "test", root, FileType::Folder)
                .unwrap();

            let mut modified_doc = initial_doc.clone();
            modified_doc.decrypted_name += " 2";

            tx.insert_metadatum(&core.config, RepoSource::Base, &modified_doc)
                .unwrap();

            initial_doc
        })
        .unwrap();

    assert_eq!(core.get_file_by_id(initial_doc.id).unwrap().name(), "test");

    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn maybe_get_metadata() {
    let core = test_core_with_account();

    core.db
        .transaction(|tx| {
            let root = tx.root().unwrap().id;
            assert_matches!(tx.maybe_get_metadata(RepoSource::Local, root).unwrap(), Some(_));
            assert_matches!(tx.maybe_get_metadata(RepoSource::Base, root).unwrap(), Some(_));
            assert_matches!(
                tx.maybe_get_metadata(RepoSource::Base, Uuid::new_v4())
                    .unwrap(),
                None
            );
            assert_matches!(
                tx.maybe_get_metadata(RepoSource::Local, Uuid::new_v4())
                    .unwrap(),
                None
            );
        })
        .unwrap()
}

#[test]
fn insert_document() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap().id;

    let document = files::create(FileType::Document, root, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document, b"content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn get_document() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document, b"content")
                .unwrap();
            assert_eq!(
                file_service::get_document(&core.config, RepoSource::Local, &document).unwrap(),
                b"content"
            );
        })
        .unwrap();

    assert_eq!(core.read_document(document.id).unwrap(), b"content");
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn get_document_nonexistent() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();

    let result = file_service::get_document(
        &core.config,
        RepoSource::Local,
        &files::create(
            FileType::Document,
            files::create_root(account).id,
            "asdf",
            &account.public_key(),
        ),
    );

    assert!(result.is_err());
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn get_document_local_falls_back_to_base() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let result = core
        .db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"content")
                .unwrap();
            file_service::get_document(&core.config, RepoSource::Local, &document).unwrap()
        })
        .unwrap();

    assert_eq!(result, b"content");
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn get_document_local_prefers_local() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content 2")
                .unwrap();
            assert_eq!(
                file_service::get_document(&core.config, RepoSource::Local, &document).unwrap(),
                b"document content 2"
            );
        })
        .unwrap();

    assert_eq!(core.read_document(document.id).unwrap(), b"document content 2");
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn maybe_get_document() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content 2")
                .unwrap();
            assert_eq!(
                file_service::maybe_get_document(&core.config, RepoSource::Local, &document)
                    .unwrap(),
                Some(b"document content 2".to_vec())
            );
            let mut document = document;
            document.id = Uuid::new_v4();
            assert_eq!(
                file_service::maybe_get_document(&core.config, RepoSource::Local, &document)
                    .unwrap(),
                None
            );
        })
        .unwrap();

    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn no_changes() {
    let core = test_core_with_account();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn new_idempotent() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 1);
    assert_document_changes_count!(core, 1);
    assert!(core
        .db
        .transaction(|tx| tx.get_all_metadata_changes().unwrap())
        .unwrap()[0]
        .old_parent_and_name
        .is_none());
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn matching_base_and_local() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn matching_local_and_base() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Folder, root.id, "dir", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn move_unmove() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let mut document =
        files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    document.parent = folder.id;
    core.db
        .transaction(|tx| tx.insert_metadatum(&core.config, RepoSource::Local, &document))
        .unwrap()
        .unwrap();

    assert_metadata_changes_count!(core, 1);
    assert_document_changes_count!(core, 0);
    assert!(core
        .db
        .transaction(|tx| tx.get_all_metadata_changes().unwrap())
        .unwrap()[0]
        .old_parent_and_name
        .is_some());
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    document.parent = root.id;
    core.db
        .transaction(|tx| tx.insert_metadatum(&core.config, RepoSource::Local, &document))
        .unwrap()
        .unwrap();
    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn rename_unrename() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let mut document =
        files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    document.decrypted_name = String::from("document 2");
    core.db
        .transaction(|tx| tx.insert_metadatum(&core.config, RepoSource::Local, &document))
        .unwrap()
        .unwrap();

    assert_metadata_changes_count!(core, 1);
    assert_document_changes_count!(core, 0);
    assert!(core
        .db
        .transaction(|tx| tx.get_all_metadata_changes().unwrap())
        .unwrap()[0]
        .old_parent_and_name
        .is_some());
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    document.decrypted_name = String::from("document");
    core.db
        .transaction(|tx| tx.insert_metadatum(&core.config, RepoSource::Local, &document))
        .unwrap()
        .unwrap();
    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn delete() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let mut document =
        files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    document.deleted = true;
    core.db
        .transaction(|tx| tx.insert_metadatum(&core.config, RepoSource::Local, &document))
        .unwrap()
        .unwrap();

    assert_metadata_changes_count!(core, 1);
    assert_document_changes_count!(core, 0);
    assert!(core
        .db
        .transaction(|tx| tx.get_all_metadata_changes().unwrap())
        .unwrap()[0]
        .old_parent_and_name
        .is_some());
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn multiple_metadata_edits() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let mut document =
        files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    folder.deleted = true;
    document.parent = folder.id;
    let document2 = files::create(FileType::Document, root.id, "document 2", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document2)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 3);
    assert_document_changes_count!(core, 1);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 4);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn document_edit() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| tx.insert_metadatum(&core.config, RepoSource::Base, &document))
        .unwrap()
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    core.db
        .transaction(|tx| {
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content")
        })
        .unwrap()
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 1);
    assert_eq!(
        core.db
            .transaction(|tx| tx.get_all_with_document_changes(&core.config).unwrap())
            .unwrap()[0],
        document.id
    );
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn document_edit_idempotent() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap()
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
    core.db
        .transaction(|tx| {
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content")
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content")
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 1);
    assert_eq!(
        core.db
            .transaction(|tx| tx.get_all_with_document_changes(&core.config).unwrap())
            .unwrap()[0],
        document.id
    );
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn document_edit_revert() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    core.db
        .transaction(|tx| {
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content 2")
        })
        .unwrap()
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 1);
    assert_eq!(
        core.db
            .transaction(|tx| tx.get_all_with_document_changes(&core.config).unwrap())
            .unwrap()[0],
        document.id
    );
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    core.db
        .transaction(|tx| {
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content")
        })
        .unwrap()
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn document_edit_manual_promote() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
    core.db
        .transaction(|tx| {
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content 2")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 1);
    assert_eq!(
        core.db
            .transaction(|tx| tx.get_all_with_document_changes(&core.config).unwrap())
            .unwrap()[0],
        document.id
    );
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
    core.db
        .transaction(|tx| {
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content 2")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn promote() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let mut document =
        files::create(FileType::Document, folder.id, "document", &account.public_key());
    let document2 =
        files::create(FileType::Document, folder.id, "document 2", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document2)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document2, b"document 2 content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 4);
    assert_metadata_count!(core, RepoSource::Local, 4);
    assert_document_count!(core, RepoSource::Base, 2);
    assert_document_count!(core, RepoSource::Local, 2);

    folder.deleted = true;
    document.parent = root.id;
    let document3 = files::create(FileType::Document, root.id, "document 3", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document3)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content 2")
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Local, &document3, b"document 3 content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 3);
    assert_document_changes_count!(core, 2);
    assert_metadata_count!(core, RepoSource::Base, 4);
    assert_metadata_count!(core, RepoSource::Local, 5);
    assert_document_count!(core, RepoSource::Base, 2);
    assert_document_count!(core, RepoSource::Local, 3);

    core.db
        .transaction(|tx| {
            tx.promote_metadata().unwrap();
            tx.promote_documents(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_eq!(core, RepoSource::Base, root.id, root);
    assert_metadata_eq!(core, RepoSource::Base, folder.id, folder);
    assert_metadata_eq!(core, RepoSource::Base, document.id, document);
    assert_metadata_eq!(core, RepoSource::Base, document2.id, document2);
    assert_metadata_eq!(core, RepoSource::Base, document3.id, document3);
    assert_document_eq!(core, RepoSource::Base, &document, b"document content 2");
    assert_document_eq!(core, RepoSource::Base, &document2, b"document 2 content");
    assert_document_eq!(core, RepoSource::Base, &document3, b"document 3 content");
    assert_metadata_count!(core, RepoSource::Base, 5);
    assert_metadata_count!(core, RepoSource::Local, 5);
    assert_document_count!(core, RepoSource::Base, 3);
    assert_document_count!(core, RepoSource::Local, 3);
}

#[test]
fn prune_deleted() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut document =
        files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    document.deleted = true;
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_nonexistent!(core, RepoSource::Base, document.id);
    assert_metadata_nonexistent!(core, RepoSource::Local, document.id);
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn prune_deleted_document_edit() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut document =
        files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    document.deleted = true;

    core.db
        .transaction(|tx| {
            tx.insert_document(&core.config, RepoSource::Local, &document, b"document content 2")
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_nonexistent!(core, RepoSource::Base, document.id);
    assert_metadata_nonexistent!(core, RepoSource::Local, document.id);
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn prune_deleted_document_in_deleted_folder() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, folder.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    folder.deleted = true;
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_nonexistent!(core, RepoSource::Base, folder.id);
    assert_metadata_nonexistent!(core, RepoSource::Local, folder.id);
    assert_metadata_nonexistent!(core, RepoSource::Base, document.id);
    assert_metadata_nonexistent!(core, RepoSource::Local, document.id);
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn prune_deleted_document_moved_from_deleted_folder() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let mut document =
        files::create(FileType::Document, folder.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();
    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    folder.deleted = true;
    document.parent = root.id;

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_nonexistent!(core, RepoSource::Base, folder.id);
    assert_metadata_nonexistent!(core, RepoSource::Local, folder.id);
    assert_metadata_eq!(core, RepoSource::Base, document.id, document);
    assert_metadata_eq!(core, RepoSource::Local, document.id, document);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn prune_deleted_base_only() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut document =
        files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    let mut document_local = document.clone();
    document_local.decrypted_name = String::from("renamed document");

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &document_local)
                .unwrap();
        })
        .unwrap();
    document.deleted = true;
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 1);
    assert_document_changes_count!(core, 0);
    assert_metadata_eq!(core, RepoSource::Base, document.id, document);
    assert_metadata_eq!(core, RepoSource::Local, document.id, document_local);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn prune_deleted_local_only() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    let mut document_deleted = document.clone();
    document_deleted.deleted = true;
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &document_deleted)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 1);
    assert_document_changes_count!(core, 0);
    assert_metadata_eq!(core, RepoSource::Base, document.id, document);
    assert_metadata_eq!(core, RepoSource::Local, document.id, document_deleted);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn prune_deleted_document_moved_from_deleted_folder_local_only() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, folder.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    let mut folder_deleted = folder;
    folder_deleted.deleted = true;
    let mut document_moved = document.clone();
    document_moved.parent = root.id;
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &folder_deleted)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &folder_deleted)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document_moved)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 1);
    assert_document_changes_count!(core, 0);
    assert_metadata_eq!(core, RepoSource::Base, document.id, document);
    assert_metadata_eq!(core, RepoSource::Local, document.id, document_moved);
    assert_metadata_count!(core, RepoSource::Base, 3);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn prune_deleted_new_local_deleted_folder() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);

    let mut deleted_folder =
        files::create(FileType::Folder, root.id, "folder", &account.public_key());
    deleted_folder.deleted = true;
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &deleted_folder)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 1);
    assert_metadata_count!(core, RepoSource::Local, 1);
    assert_document_count!(core, RepoSource::Base, 0);
    assert_document_count!(core, RepoSource::Local, 0);
}

#[test]
fn prune_deleted_new_local_deleted_folder_with_existing_moved_child() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    let mut deleted_folder =
        files::create(FileType::Folder, root.id, "folder", &account.public_key());
    deleted_folder.deleted = true;
    let mut document_moved = document.clone();
    document_moved.parent = deleted_folder.id;

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &deleted_folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document_moved)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 2);
    assert_document_changes_count!(core, 0);
    assert_metadata_eq!(core, RepoSource::Local, document.id, document_moved);
    assert_metadata_eq!(core, RepoSource::Local, deleted_folder.id, deleted_folder);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}

#[test]
fn prune_deleted_new_local_deleted_folder_with_deleted_existing_moved_child() {
    let core = test_core_with_account();
    let account = &core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Base, &document)
                .unwrap();
            tx.insert_document(&core.config, RepoSource::Base, &document, b"document content")
                .unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 0);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 2);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);

    let mut deleted_folder =
        files::create(FileType::Folder, root.id, "folder", &account.public_key());
    deleted_folder.deleted = true;
    let mut document_moved_and_deleted = document;
    document_moved_and_deleted.parent = deleted_folder.id;
    document_moved_and_deleted.deleted = true;
    core.db
        .transaction(|tx| {
            tx.insert_metadatum(&core.config, RepoSource::Local, &deleted_folder)
                .unwrap();
            tx.insert_metadatum(&core.config, RepoSource::Local, &document_moved_and_deleted)
                .unwrap();
            tx.prune_deleted(&core.config).unwrap();
        })
        .unwrap();

    assert_metadata_changes_count!(core, 2);
    assert_document_changes_count!(core, 0);
    assert_metadata_count!(core, RepoSource::Base, 2);
    assert_metadata_count!(core, RepoSource::Local, 3);
    assert_document_count!(core, RepoSource::Base, 1);
    assert_document_count!(core, RepoSource::Local, 1);
}
