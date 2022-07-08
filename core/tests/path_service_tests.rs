use lockbook_core::model::errors::CreateFileAtPathError::*;
use lockbook_core::model::errors::GetPathByIdError::*;
use lockbook_core::service::path_service::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use lockbook_core::CreateLinkAtPathError;
use lockbook_core::Error::UiError;
use lockbook_core::ShareMode;
use lockbook_core::Uuid;
use lockbook_models::file_metadata::FileType;
use test_utils::*;

#[test]
fn create_at_path_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let doc = core
        .create_at_path(&format!("{}/document", &account.username))
        .unwrap();

    assert_eq!(doc.file_type, FileType::Document);
}

#[test]
fn create_at_path_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
}

#[test]
fn create_at_path_in_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();
    let document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_missing_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let folder = core
        .get_by_path(&format!("{}/folder", &account.username))
        .unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_missing_folders() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let document = core
        .create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    let folder1 = core
        .get_by_path(&format!("{}/folder", &account.username))
        .unwrap();
    let folder2 = core
        .get_by_path(&format!("{}/folder/folder", &account.username))
        .unwrap();

    assert_eq!(folder1.file_type, FileType::Folder);
    assert_eq!(folder2.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_path_contains_empty_file_name() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let result = core.create_at_path(&format!("{}//document", &account.username));

    assert_matches!(result, Err(UiError(PathContainsEmptyFile)));
}

#[test]
fn create_at_path_path_starts_with_non_root() {
    let core = test_core_with_account();

    let result = core.create_at_path(&format!("{}/folder/document", "not-account-username"));

    assert_matches!(result, Err(UiError(PathDoesntStartWithRoot)));
}

#[test]
fn create_at_path_path_taken() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    core.create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let result = core.create_at_path(&format!("{}/folder/document", &account.username));

    assert_matches!(result, Err(UiError(FileAlreadyExists)));
}

#[test]
fn create_at_path_not_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    core.create_at_path(&format!("{}/not-folder", &account.username))
        .unwrap();
    let result = core.create_at_path(&format!("{}/not-folder/document", &account.username));

    assert_matches!(result, Err(UiError(DocumentTreatedAsFolder)));
}

#[test]
fn create_at_path_insufficient_permission() {
    let core1 = test_core_with_account();
    let account1 = core1.get_account().unwrap();

    let core2 = test_core_with_account();
    let account2 = core2.get_account().unwrap();
    let folder = core2
        .create_at_path(&format!("{}/shared-folder/", &account2.username))
        .unwrap();
    core2
        .share_file(folder.id, &account1.username, ShareMode::Read)
        .unwrap();
    core2.sync(None).unwrap();

    core1.sync(None).unwrap();
    core1
        .create_link_at_path(&format!("{}/received-folder", &account1.username), folder.id)
        .unwrap();
    let result = core1.create_at_path(&format!("{}/received-folder/document", &account1.username));

    assert_matches!(result, Err(UiError(InsufficientPermission)));
}

#[test]
fn get_by_path_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let created_document = core
        .create_at_path(&format!("{}/document", &account.username))
        .unwrap();
    let document = core
        .get_by_path(&format!("{}/document", &account.username))
        .unwrap();

    assert_eq!(created_document, document);
}

#[test]
fn get_by_path_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let created_folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();
    let folder = core
        .get_by_path(&format!("{}/folder", &account.username))
        .unwrap();

    assert_eq!(created_folder, folder);
}

#[test]
fn get_by_path_document_in_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let created_document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let document = core
        .get_by_path(&format!("{}/folder/document", &account.username))
        .unwrap();

    assert_eq!(created_document, document);
}

#[test]
fn get_path_by_id_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let document = core
        .create_at_path(&format!("{}/document", &account.username))
        .unwrap();
    let document_path = core.get_path_by_id(document.id).unwrap();

    assert_eq!(&document_path, &format!("{}/document", &account.username));
}

#[test]
fn get_path_by_id_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();
    let folder_path = core.get_path_by_id(folder.id).unwrap();

    assert_eq!(&folder_path, &format!("{}/folder/", &account.username));
}

#[test]
fn get_path_by_id_link() {
    let core1 = test_core_with_account();
    let account1 = core1.get_account().unwrap();

    let core2 = test_core_with_account();
    let account2 = core2.get_account().unwrap();
    let folder = core2
        .create_at_path(&format!("{}/shared-folder/", &account2.username))
        .unwrap();
    core2
        .share_file(folder.id, &account1.username, ShareMode::Read)
        .unwrap();
    core2.sync(None).unwrap();

    core1.sync(None).unwrap();
    let link = core1
        .create_link_at_path(&format!("{}/received-folder", &account1.username), folder.id)
        .unwrap();
    let result = core1.get_path_by_id(link.id);

    assert_matches!(result, Err(UiError(FileIsLink)));
}

#[test]
fn get_path_by_id_document_in_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let document_path = core.get_path_by_id(document.id).unwrap();

    assert_eq!(&document_path, &format!("{}/folder/document", &account.username));
}

#[test]
fn get_all_paths() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(None).unwrap();
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
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(Some(DocumentsOnly)).unwrap();
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
    assert_eq!(all_paths.len(), 1);
}

#[test]
fn get_all_paths_folders_only() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(Some(FoldersOnly)).unwrap();
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
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(Some(LeafNodesOnly)).unwrap();
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
    assert_eq!(all_paths.len(), 2);
}

#[test]
fn create_link_at_path_target_is_owned() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = core
        .create_file("document0", root.id, FileType::Document)
        .unwrap();

    let result = core.create_link_at_path(&format!("{}/link", &account.username), document.id);
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::LinkTargetIsOwned)));
}

#[test]
fn create_link_at_path_target_nonexistent() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let result = core.create_link_at_path(&format!("{}/link", &account.username), Uuid::new_v4());
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::LinkTargetNonexistent)));
}

#[test]
fn create_link_at_path_link_in_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document0 = cores[0]
        .create_file("document0", roots[0].id, FileType::Document)
        .unwrap();
    let folder0 = cores[0]
        .create_file("folder0", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(document0.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { linked_file: folder0.id })
        .unwrap();

    let result = cores[1].create_link_at_path(
        &format!("{}/folder_link/document", &accounts[1].username),
        document0.id,
    );
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::LinkInSharedFolder)));
}

#[test]
fn create_link_at_path_link_duplicate() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document0 = cores[0]
        .create_file("document0", roots[0].id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(document0.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_link_at_path(&format!("{}/link1", &accounts[1].username), document0.id)
        .unwrap();
    let result =
        cores[1].create_link_at_path(&format!("{}/link2", &accounts[1].username), document0.id);
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::MultipleLinksToSameFile)));
}
