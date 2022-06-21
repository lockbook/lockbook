use hmdb::transaction::Transaction;
use rand::Rng;
use uuid::Uuid;

use lockbook_core::model::errors::TestRepoError::*;
use lockbook_core::model::errors::Warning::*;
use lockbook_core::model::repo::RepoSource;
use lockbook_core::pure_functions::files;
use lockbook_models::file_metadata::FileType::Document;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
use lockbook_models::tree::{FileMetaMapExt, FileMetaVecExt, TestFileTreeError};
use test_utils::*;

#[test]
fn test_integrity_no_problems() {
    let core = test_core_with_account();
    core.validate().unwrap();
}

#[test]
fn test_integrity_no_problems_but_more_complicated() {
    let core = test_core_with_account();
    core.create_at_path(&path(&core, "test.md")).unwrap();
    core.validate().unwrap();
}

#[test]
fn test_no_account() {
    let core = test_core();
    assert_matches!(core.validate(), Err(NoAccount));
}

#[test]
fn test_no_root() {
    let core = test_core_with_account();
    core.db.transaction(|tx| tx.base_metadata.clear()).unwrap();
    core.db.transaction(|tx| tx.root.clear()).unwrap();
    assert_matches!(core.validate(), Err(NoRootFolder));
}

#[test]
fn test_orphaned_children() {
    let core = test_core_with_account();

    core.create_at_path(&path(&core, "folder1/folder2/document1.md"))
        .unwrap();
    core.validate().unwrap();

    let parent = core.get_by_path(&path(&core, "folder1")).unwrap().id;
    core.db.local_metadata.delete(parent).unwrap();
    assert_matches!(core.validate(), Err(FileOrphaned(_)));
}

#[test]
fn test_invalid_file_name_slash() {
    let core = test_core_with_account();
    let doc = core.create_at_path(&path(&core, "document1.md")).unwrap();
    core.db
        .transaction(|tx| {
            let mut ctx = core.context(tx).unwrap();
            let mut doc = ctx.get_metadata(RepoSource::Local, doc.id).unwrap();
            doc.decrypted_name = String::from("na/me.md");
            ctx.insert_metadatum(&core.config, RepoSource::Local, &doc)
                .unwrap();
        })
        .unwrap();

    assert_matches!(core.validate(), Err(FileNameContainsSlash(_)));
}

#[test]
fn test_invalid_file_name_empty() {
    let core = test_core_with_account();
    let mut doc = core.create_at_path(&path(&core, "document1.md")).unwrap();
    core.db
        .transaction(|tx| {
            let mut ctx = core.context(tx).unwrap();
            ctx.get_metadata(RepoSource::Local, doc.id).unwrap();
            doc.decrypted_name = String::from("");
            ctx.insert_metadatum(&core.config, RepoSource::Local, &doc)
                .unwrap();
        })
        .unwrap();

    assert_matches!(core.validate(), Err(FileNameEmpty(_)));
}

#[test]
fn test_cycle() {
    let core = test_core_with_account();
    core.create_at_path(&path(&core, "folder1/folder2/document1.md"))
        .unwrap();
    let parent = core.get_by_path(&path(&core, "folder1")).unwrap().id;
    let mut parent = core.db.local_metadata.get(&parent).unwrap().unwrap();
    let child = core.get_by_path(&path(&core, "folder1/folder2")).unwrap();
    parent.parent = child.id;
    core.db.local_metadata.insert(parent.id, parent).unwrap();
    assert_matches!(core.validate(), Err(CycleDetected(_)));
}

#[test]
fn test_cycle_with_three_files() {
    let core = test_core_with_account();
    let folder3 = core
        .create_at_path(&path(&core, "folder1/folder2/folder3/"))
        .unwrap();
    let parent = core.get_by_path(&path(&core, "folder1")).unwrap();
    let mut parent = core.db.local_metadata.get(&parent.id).unwrap().unwrap();
    parent.parent = folder3.id;
    core.db.local_metadata.insert(parent.id, parent).unwrap();
    assert_matches!(core.validate(), Err(CycleDetected(_)));
}

#[test]
fn test_documents_treated_as_folders() {
    let core = test_core_with_account();
    core.create_at_path(&path(&core, "folder1/folder2/document1.md"))
        .unwrap();
    let parent = core.get_by_path(&path(&core, "folder1")).unwrap();
    let mut parent = core.db.local_metadata.get(&parent.id).unwrap().unwrap();
    parent.file_type = Document;
    core.db.local_metadata.insert(parent.id, parent).unwrap();
    assert_matches!(core.validate(), Err(DocumentTreatedAsFolder(_)));
}

#[test]
fn test_name_conflict() {
    let core = test_core_with_account();
    let doc = core.create_at_path(&path(&core, "document1.md")).unwrap();
    core.create_at_path(&path(&core, "document2.md")).unwrap();
    core.db
        .transaction(|tx| {
            let mut ctx = core.context(tx).unwrap();
            let mut doc = ctx.get_metadata(RepoSource::Local, doc.id).unwrap();
            doc.decrypted_name = String::from("document2.md");
            ctx.insert_metadatum(&core.config, RepoSource::Local, &doc)
                .unwrap();
        })
        .unwrap();
    assert_matches!(core.validate(), Err(NameConflictDetected(_)));
}

#[test]
fn test_empty_file() {
    let core = test_core_with_account();
    let doc = core.create_at_path(&path(&core, "document.txt")).unwrap();
    core.write_document(doc.id, &[]).unwrap();
    let warnings = core.validate();

    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([EmptyFile(_)]));
}

#[test]
fn test_invalid_utf8() {
    let core = test_core_with_account();
    let doc = core.create_at_path(&path(&core, "document.txt")).unwrap();
    core.write_document(doc.id, rand::thread_rng().gen::<[u8; 32]>().as_ref())
        .unwrap();
    let warnings = core.validate();
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([InvalidUTF8(_)]));
}

#[test]
fn test_invalid_utf8_ignores_non_utf_file_extensions() {
    let core = test_core_with_account();
    let doc = core.create_at_path(&path(&core, "document.png")).unwrap();
    core.write_document(doc.id, rand::thread_rng().gen::<[u8; 32]>().as_ref())
        .unwrap();
    let warnings = core.validate();
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([]));
}

#[test]
fn test_invalid_drawing() {
    let core = test_core_with_account();
    let doc = core.create_at_path(&path(&core, "document.draw")).unwrap();
    core.write_document(doc.id, rand::thread_rng().gen::<[u8; 32]>().as_ref())
        .unwrap();
    let warnings = core.validate();
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([UnreadableDrawing(_)]));
}

#[test]
fn test_file_tree_integrity_empty() {
    let files: Vec<DecryptedFileMetadata> = vec![];
    let result = files.to_map().verify_integrity();

    assert_eq!(result, Ok(()));
}

#[test]
fn test_file_tree_integrity_nonempty_ok() {
    let account = test_core_with_account().get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, folder.id, "document", &account.public_key());

    let result = [root, folder, document].to_map().verify_integrity();

    assert_eq!(result, Ok(()));
}

#[test]
fn test_file_tree_integrity_no_root() {
    let account = test_core_with_account().get_account().unwrap();
    let mut root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, folder.id, "document", &account.public_key());
    root.parent = folder.id;

    let result = [root, folder, document].to_map().verify_integrity();

    assert_eq!(result, Err(TestFileTreeError::NoRootFolder));
}

#[test]
fn test_file_tree_integrity_orphan() {
    let account = test_core_with_account().get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let mut document =
        files::create(FileType::Document, folder.id, "document", &account.public_key());
    document.parent = Uuid::new_v4();
    let document_id = document.id;

    let result = [root, folder, document].to_map().verify_integrity();

    assert_eq!(result, Err(TestFileTreeError::FileOrphaned(document_id)));
}

#[test]
fn test_file_tree_integrity_1cycle() {
    let account = test_core_with_account().get_account().unwrap();
    let root = files::create_root(&account);
    let mut folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    folder.parent = folder.id;

    let result = [root, folder].to_map().verify_integrity();

    assert_matches!(result, Err(TestFileTreeError::CycleDetected(_)));
}

#[test]
fn test_file_tree_integrity_2cycle() {
    let account = test_core_with_account().get_account().unwrap();
    let root = files::create_root(&account);
    let mut folder1 = files::create(FileType::Folder, root.id, "folder1", &account.public_key());
    let mut folder2 = files::create(FileType::Folder, root.id, "folder2", &account.public_key());
    folder1.parent = folder2.id;
    folder2.parent = folder1.id;

    let result = [root, folder1, folder2].to_map().verify_integrity();

    assert_matches!(result, Err(TestFileTreeError::CycleDetected(_)));
}

#[test]
fn test_file_tree_integrity_document_treated_as_folder() {
    let account = test_core_with_account().get_account().unwrap();
    let root = files::create_root(&account);
    let document1 = files::create(FileType::Document, root.id, "document1", &account.public_key());
    let document2 =
        files::create(FileType::Document, document1.id, "document2", &account.public_key());
    let document1_id = document1.id;

    let result = [root, document1, document2].to_map().verify_integrity();

    assert_eq!(result, Err(TestFileTreeError::DocumentTreatedAsFolder(document1_id)));
}

#[test]
fn test_file_tree_integrity_path_conflict() {
    let account = test_core_with_account().get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "file", &account.public_key());
    let document = files::create(FileType::Document, root.id, "file", &account.public_key());

    let result = [root, folder, document].to_map().verify_integrity();

    assert_matches!(result, Err(TestFileTreeError::NameConflictDetected(_)));
}
