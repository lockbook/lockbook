use lockbook_core::pure_functions::files;
use lockbook_core::CoreError;
use lockbook_models::file_metadata::FileType;
use lockbook_models::tree::{FileMetaExt, PathConflict};
use test_utils::test_core_with_account;

fn apply_move_path_conflict() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document1 = files::create(FileType::Document, root.id, "document", &account.public_key());
    let document2 = files::create(FileType::Document, folder.id, "document", &account.public_key());

    let folder_id = folder.id;
    let document1_id = document1.id;
    // don't know what to do here yet
    let result = files::apply_move(&[root, folder, document1, document2], document1_id, folder_id);
    assert_eq!(result, Err(CoreError::PathTaken));
}