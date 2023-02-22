use lockbook_core::model::errors::*;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::path_ops::Filter;
use test_utils::*;

#[test]
fn test_create_delete_list() {
    let core = test_core_with_account();
    let id = core.create_at_path("test.md").unwrap().id;
    assert_eq!(core.list_paths(Some(Filter::LeafNodesOnly)).unwrap().len(), 1);
    core.delete_file(id).unwrap();
    assert_eq!(core.list_paths(Some(Filter::LeafNodesOnly)).unwrap().len(), 0);
}

#[test]
fn test_create_delete_read() {
    let core = test_core_with_account();
    let id = core.create_at_path("test.md").unwrap().id;
    core.delete_file(id).unwrap();
    assert_matches!(core.read_document(id), Err(CoreError::FileNonexistent));
}

#[test]
fn test_create_delete_write() {
    let core = test_core_with_account();
    let id = core.create_at_path("test.md").unwrap().id;
    core.delete_file(id).unwrap();
    assert_matches!(
        core.write_document(id, "document content".as_bytes()),
        Err(CoreError::FileNonexistent)
    );
}

#[test]
fn test_create_parent_delete_create_in_parent() {
    let core = test_core_with_account();
    let id = core.create_at_path("folder/").unwrap().id;
    core.delete_file(id).unwrap();

    assert_matches!(
        core.create_file("document", id, FileType::Document),
        Err(CoreError::FileParentNonexistent)
    );
}

#[test]
fn try_to_delete_root() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();
    assert_matches!(core.delete_file(root.id), Err(CoreError::RootModificationInvalid));
}

#[test]
fn test_create_parent_delete_parent_read_doc() {
    let core = test_core_with_account();
    let doc = core.create_at_path("folder/test.md").unwrap();
    core.write_document(doc.id, "content".as_bytes()).unwrap();
    assert_eq!(core.read_document(doc.id).unwrap(), "content".as_bytes());
    core.delete_file(doc.parent).unwrap();
    assert_matches!(core.read_document(doc.id), Err(CoreError::FileNonexistent));
}

#[test]
fn test_create_parent_delete_parent_rename_doc() {
    let core = test_core_with_account();
    let doc = core.create_at_path("folder/test.md").unwrap();
    core.delete_file(doc.parent).unwrap();
    assert_matches!(core.rename_file(doc.id, "test2.md"), Err(CoreError::FileNonexistent));
}

#[test]
fn test_create_parent_delete_parent_rename_parent() {
    let core = test_core_with_account();
    let doc = core.create_at_path("folder/test.md").unwrap();
    core.delete_file(doc.parent).unwrap();
    assert_matches!(core.rename_file(doc.parent, "folder2"), Err(CoreError::FileNonexistent));
}

#[test]
fn test_folder_move_delete_source_doc() {
    let core = test_core_with_account();
    let doc = core.create_at_path("folder/test.md").unwrap();
    let folder2 = core.create_at_path("folder2/").unwrap();
    core.delete_file(doc.parent).unwrap();
    assert_matches!(core.move_file(doc.id, folder2.id), Err(CoreError::FileNonexistent));
}

#[test]
fn test_folder_move_delete_source_parent() {
    let core = test_core_with_account();
    let doc = core.create_at_path("folder/test.md").unwrap();
    let folder2 = core.create_at_path("folder2/").unwrap();
    core.delete_file(doc.parent).unwrap();
    assert_matches!(core.move_file(doc.parent, folder2.id), Err(CoreError::FileNonexistent));
}

#[test]
fn test_folder_move_delete_destination_parent() {
    let core = test_core_with_account();
    let doc = core.create_at_path("folder/test.md").unwrap();
    let folder2 = core.create_at_path("folder2/").unwrap();
    core.delete_file(folder2.id).unwrap();
    assert_matches!(core.move_file(doc.id, folder2.id), Err(CoreError::FileParentNonexistent));
}

#[test]
fn test_folder_move_delete_destination_doc() {
    let core = test_core_with_account();
    let doc = core.create_at_path("folder/test.md").unwrap();
    let folder2 = core.create_at_path("folder2/").unwrap();
    core.delete_file(folder2.id).unwrap();
    assert_matches!(core.move_file(doc.parent, folder2.id), Err(CoreError::FileParentNonexistent));
}

#[test]
fn test_delete_list_files() {
    let core = test_core_with_account();
    let f1 = core.create_at_path("f1/").unwrap();
    core.create_at_path("f1/f2/").unwrap();
    let d1 = core.create_at_path("f1/f2/d1.md").unwrap();
    core.delete_file(f1.id).unwrap();

    let mut files = core.list_metadatas().unwrap();
    files.retain(|meta| meta.id == d1.id);

    assert!(files.is_empty());
}

#[test]
fn test_write_delete_sync_doc() {
    let core = test_core_with_account();

    let doc = core.create_at_path("test.md").unwrap().id;
    core.write_document(doc, &[1, 2, 3]).unwrap();
    core.delete_file(doc).unwrap();
    core.sync(None).unwrap();
}
