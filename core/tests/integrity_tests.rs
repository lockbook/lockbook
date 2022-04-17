mod test_utils;

use hmdb::transaction::Transaction;
use rand::Rng;

use crate::test_utils::{path, test_core, test_core_with_account};
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
    let core = test_core_with_account();
    core.db
        .transaction(|tx| tx.test_repo_integrity(&core.config))
        .unwrap()
        .unwrap();
}

#[test]
fn test_integrity_no_problems_but_more_complicated() {
    let core = test_core_with_account();
    core.create_at_path(&path(&core, "test.md")).unwrap();
    core.db
        .transaction(|tx| tx.test_repo_integrity(&core.config))
        .unwrap()
        .unwrap();
}

#[test]
fn test_no_account() {
    let core = test_core();

    assert_matches!(
        core.db
            .transaction(|tx| tx.test_repo_integrity(&core.config))
            .unwrap(),
        Err(NoAccount)
    );
}

#[test]
fn test_no_root() {
    let core = test_core();
    core.db.transaction(|tx| tx.base_metadata.clear()).unwrap();
    assert_matches!(
        core.db
            .transaction(|tx| tx.test_repo_integrity(&core.config))
            .unwrap(),
        Err(NoRootFolder)
    );
}

#[test]
fn test_orphaned_children() {
    let cfg = test_utils::test_config();
    let (_account, root) = test_utils::create_account(&cfg);
    create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/document1.md")).unwrap();

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

    assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(FileNameContainsSlash(_)));
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
    create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/document1.md")).unwrap();
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
        create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/folder3/")).unwrap();

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
    create_file_at_path(&cfg, &test_utils::path(&root, "/folder1/folder2/document1.md")).unwrap();
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

    assert_matches!(integrity_service::test_repo_integrity(&cfg), Err(DocumentTreatedAsFolder(_)));
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

#[cfg(test)]
mod unit_tests {
    use crate::assert_matches;
    use crate::{
        pure_functions::files,
        service::{integrity_service::TestFileTreeError, test_utils},
    };
    use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
    use lockbook_models::tree::FileMetaExt;
    use uuid::Uuid;

    #[test]
    fn test_file_tree_integrity_empty() {
        let files: Vec<DecryptedFileMetadata> = vec![];
        let result = files.verify_integrity();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_file_tree_integrity_nonempty_ok() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, folder.id, "document", &account.public_key());

        let result = [root, folder, document].verify_integrity();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_file_tree_integrity_no_root() {
        let account = test_utils::generate_account();
        let mut root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, folder.id, "document", &account.public_key());
        root.parent = folder.id;

        let result = [root, folder, document].verify_integrity();

        assert_eq!(result, Err(TestFileTreeError::NoRootFolder));
    }

    #[test]
    fn test_file_tree_integrity_orphan() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let mut document =
            files::create(FileType::Document, folder.id, "document", &account.public_key());
        document.parent = Uuid::new_v4();
        let document_id = document.id;

        let result = [root, folder, document].verify_integrity();

        assert_eq!(result, Err(TestFileTreeError::FileOrphaned(document_id)));
    }

    #[test]
    fn test_file_tree_integrity_1cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        folder.parent = folder.id;

        let result = [root, folder].verify_integrity();

        assert_matches!(result, Err(TestFileTreeError::CycleDetected(_)));
    }

    #[test]
    fn test_file_tree_integrity_2cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let mut folder1 =
            files::create(FileType::Folder, root.id, "folder1", &account.public_key());
        let mut folder2 =
            files::create(FileType::Folder, root.id, "folder2", &account.public_key());
        folder1.parent = folder2.id;
        folder2.parent = folder1.id;

        let result = [root, folder1, folder2].verify_integrity();

        assert_matches!(result, Err(TestFileTreeError::CycleDetected(_)));
    }

    #[test]
    fn test_file_tree_integrity_document_treated_as_folder() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document1 =
            files::create(FileType::Document, root.id, "document1", &account.public_key());
        let document2 =
            files::create(FileType::Document, document1.id, "document2", &account.public_key());
        let document1_id = document1.id;

        let result = [root, document1, document2].verify_integrity();

        assert_eq!(result, Err(TestFileTreeError::DocumentTreatedAsFolder(document1_id)));
    }

    #[test]
    fn test_file_tree_integrity_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "file", &account.public_key());
        let document = files::create(FileType::Document, root.id, "file", &account.public_key());

        let result = [root, folder, document].verify_integrity();

        assert_matches!(result, Err(TestFileTreeError::NameConflictDetected(_)));
    }
}
