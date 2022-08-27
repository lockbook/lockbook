use itertools::Itertools;
use lockbook_core::Error::UiError;
use lockbook_core::{
    CreateFileAtPathError, CreateFileError, CreateLinkAtPathError, DeletePendingShareError, Error,
    FileDeleteError, MoveFileError, RenameFileError, ShareFileError, WriteToDocumentError,
};
use lockbook_shared::file::{File, ShareMode};
use lockbook_shared::file_metadata::FileType;
use std::collections::HashMap;
use test_utils::*;
use uuid::Uuid;

#[test]
fn write_document_read_share() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let result = cores[1].write_document(document.id, b"document content");
    assert_matches!(result, Err(Error::UiError(WriteToDocumentError::InsufficientPermission)));
}

#[test]
fn write_document_in_read_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("/folder/").unwrap();
    let document = cores[0].create_at_path("/folder/document").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let result = cores[1].write_document(document.id, b"document content");
    assert_matches!(result, Err(Error::UiError(WriteToDocumentError::InsufficientPermission)));
}

#[test]
fn write_document_write_share() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    assert_eq!(cores[1].read_document(document.id).unwrap(), b"document content by sharee");
    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();
    assert_eq!(cores[0].read_document(document.id).unwrap(), b"document content by sharee");
}

#[test]
fn write_document_in_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("/folder/").unwrap();
    let document = cores[0].create_at_path("/folder/document").unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    assert_eq!(cores[1].read_document(document.id).unwrap(), b"document content by sharee");
    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();
    assert_eq!(cores[0].read_document(document.id).unwrap(), b"document content by sharee");
}

#[test]
fn write_document_in_write_shared_folder_in_read_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").unwrap();
    let document = cores[0].create_at_path("/folder/folder/document").unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    assert_eq!(cores[1].read_document(document.id).unwrap(), b"document content by sharee");
    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();
    assert_eq!(cores[0].read_document(document.id).unwrap(), b"document content by sharee");
}

#[test]
fn write_document_in_read_shared_folder_in_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").unwrap();
    let document = cores[0].create_at_path("/folder/folder/document").unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    assert_eq!(cores[1].read_document(document.id).unwrap(), b"document content by sharee");
    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();
    assert_eq!(cores[0].read_document(document.id).unwrap(), b"document content by sharee");
}

#[test]
fn write_document_write_share_by_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let link = cores[1]
        .create_file("link0", roots[1].id, FileType::Link { target: document.id })
        .unwrap();
    cores[1]
        .write_document(link.id, b"document content by sharee")
        .unwrap();
    assert_eq!(cores[1].read_document(document.id).unwrap(), b"document content by sharee");
    assert_eq!(cores[1].read_document(link.id).unwrap(), b"document content by sharee");
    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();
    assert_eq!(cores[0].read_document(document.id).unwrap(), b"document content by sharee");
}

#[test]
fn share_file_root() {
    let core = test_core_with_account();
    let sharee_core = test_core_with_account();
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let result = core.share_file(root.id, &sharee_account.username, ShareMode::Read);
    assert_matches!(result, Err(Error::UiError(ShareFileError::CannotShareRoot)));
}

#[test]
fn share_file_nonexistent() {
    let core = test_core_with_account();
    let sharee_core = test_core_with_account();
    let sharee_account = &sharee_core.get_account().unwrap();

    let result = core.share_file(Uuid::new_v4(), &sharee_account.username, ShareMode::Read);
    assert_matches!(result, Err(Error::UiError(ShareFileError::FileNonexistent)));
}

#[test]
fn share_file_in_shared_folder() {
    let core = test_core_with_account();
    let sharee_core = test_core_with_account();
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let outer_folder = core
        .create_file("outer_folder", root.id, FileType::Folder)
        .unwrap();
    let inner_folder = core
        .create_file("inner_folder", outer_folder.id, FileType::Folder)
        .unwrap();
    core.share_file(outer_folder.id, &sharee_account.username, ShareMode::Read)
        .unwrap();

    core.share_file(inner_folder.id, &sharee_account.username, ShareMode::Read)
        .unwrap();
}

#[test]
fn share_file_duplicate() {
    let core = test_core_with_account();
    let sharee_core = test_core_with_account();
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = core
        .create_file("document", root.id, FileType::Document)
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Read)
        .unwrap();

    let result = core.share_file(document.id, &sharee_account.username, ShareMode::Read);
    assert_matches!(result, Err(Error::UiError(ShareFileError::ShareAlreadyExists)));
}

#[test]
fn share_file_duplicate_new_mode() {
    let core = test_core_with_account();
    let sharee_core = test_core_with_account();
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let document = core
        .create_file("document", root.id, FileType::Document)
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Read)
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Write)
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Read)
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Write)
        .unwrap();
}

#[test]
fn share_folder_with_link_inside() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder0 = cores[0]
        .create_file("folder0", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let folder1 = cores[1]
        .create_file("folder1", roots[1].id, FileType::Folder)
        .unwrap();
    cores[1]
        .create_file("link0", folder1.id, FileType::Link { target: folder0.id })
        .unwrap();

    let result = cores[1].share_file(folder1.id, &accounts[2].username, ShareMode::Read);
    assert_matches!(result, Err(Error::UiError(ShareFileError::LinkInSharedFolder)));
}

#[test]
fn share_unowned_file_read() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder0 = cores[0]
        .create_file("folder0", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    cores[1]
        .share_file(folder0.id, &accounts[2].username, ShareMode::Read)
        .unwrap();
}

#[test]
fn share_unowned_file_write() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder0 = cores[0]
        .create_file("folder0", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    let result = cores[1].share_file(folder0.id, &accounts[2].username, ShareMode::Write);
    assert_matches!(result, Err(Error::UiError(ShareFileError::InsufficientPermission)));
}

#[test]
fn delete_pending_share() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder0 = cores[0]
        .create_file("folder0", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    cores[1].delete_pending_share(folder0.id).unwrap();
}

#[test]
fn delete_pending_share_root() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let result = core.delete_pending_share(root.id);
    assert_matches!(result, Err(Error::Unexpected(_)));
}

#[test]
fn delete_pending_share_duplicate() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder0 = cores[0]
        .create_file("folder0", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    cores[1].delete_pending_share(folder0.id).unwrap();
    let result = cores[1].delete_pending_share(folder0.id);
    assert_matches!(result, Err(Error::UiError(DeletePendingShareError::FileNonexistent)));
}

#[test]
fn delete_pending_share_nonexistent() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder0 = cores[0]
        .create_file("folder0", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    cores[1].delete_pending_share(folder0.id).unwrap();
    let result = cores[1].delete_pending_share(folder0.id);
    assert_matches!(result, Err(Error::UiError(DeletePendingShareError::FileNonexistent)));
}

#[test]
fn create_at_path_insufficient_permission() {
    let core1 = test_core_with_account();
    let account1 = core1.get_account().unwrap();

    let core2 = test_core_with_account();
    let folder = core2.create_at_path("shared-folder/").unwrap();
    core2
        .share_file(folder.id, &account1.username, ShareMode::Read)
        .unwrap();
    core2.sync(None).unwrap();

    core1.sync(None).unwrap();
    core1
        .create_link_at_path("/received-folder", folder.id)
        .unwrap();
    let result = core1.create_at_path("received-folder/document");

    assert_matches!(result, Err(UiError(CreateFileAtPathError::InsufficientPermission)));
}

#[test]
fn get_path_by_id_link() {
    let core1 = test_core_with_account();
    let account1 = core1.get_account().unwrap();

    let core2 = test_core_with_account();
    let folder = core2.create_at_path("shared-folder/").unwrap();
    core2
        .share_file(folder.id, &account1.username, ShareMode::Read)
        .unwrap();
    core2.sync(None).unwrap();

    core1.sync(None).unwrap();
    let link = core1
        .create_link_at_path("received-folder", folder.id)
        .unwrap();
    let result = core1.get_path_by_id(link.id);

    assert_matches!(result, Err(_));
}

#[test]
fn create_link_at_path_target_is_owned() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();
    let document = core
        .create_file("document0", root.id, FileType::Document)
        .unwrap();

    let result = core.create_link_at_path("link", document.id);
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::LinkTargetIsOwned)));
}

#[test]
fn create_link_at_path_target_nonexistent() {
    let core = test_core_with_account();

    let result = core.create_link_at_path("link", Uuid::new_v4());
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
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder0.id })
        .unwrap();

    let result = cores[1].create_link_at_path("folder_link/document", document0.id);
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
        .create_link_at_path("/link1", document0.id)
        .unwrap();
    let result = cores[1].create_link_at_path("/link2", document0.id);
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::MultipleLinksToSameFile)));
}

#[test]
fn create_file_link_target_nonexistent() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let result = core.create_file("link", root.id, FileType::Link { target: Uuid::new_v4() });
    assert_matches!(result, Err(UiError(CreateFileError::LinkTargetNonexistent)));
}

#[test]
fn create_file_link_target_owned() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let document = core
        .create_file("document", root.id, FileType::Document)
        .unwrap();
    let result = core.create_file("link", root.id, FileType::Link { target: document.id });
    assert_matches!(result, Err(UiError(CreateFileError::LinkTargetIsOwned)));
}

#[test]
fn create_file_shared_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0]
        .create_file("document", roots[0].id, FileType::Document)
        .unwrap();
    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let result =
        cores[1].create_file("document_link", folder.id, FileType::Link { target: document.id });
    assert_matches!(result, Err(UiError(CreateFileError::LinkInSharedFolder)));
}

#[test]
fn create_file_duplicate_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0]
        .create_file("document", roots[0].id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("link_1", roots[1].id, FileType::Link { target: document.id })
        .unwrap();
    let result =
        cores[1].create_file("link_2", roots[1].id, FileType::Link { target: document.id });
    // assert_matches!(result, Err(UiError(CreateFileError::MultipleLinksToSameFile)));
    assert_matches!(result, Err(_));
}

#[test]
fn create_file_in_read_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let result = cores[1].create_file("document", folder.id, FileType::Document);
    assert_matches!(result, Err(UiError(CreateFileError::InsufficientPermission)));
}

#[test]
fn create_file_in_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    cores[1]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();
}

#[test]
fn rename_file_in_read_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let result = cores[1].rename_file(document.id, "renamed-document");
    assert_matches!(result, Err(UiError(RenameFileError::InsufficientPermission)));
}

#[test]
fn rename_file_in_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    cores[1]
        .rename_file(document.id, "renamed-document")
        .unwrap();
}

#[test]
fn rename_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let result = cores[1].rename_file(folder.id, "renamed-folder");
    assert_matches!(result, Err(UiError(RenameFileError::InsufficientPermission)));
}

#[test]
fn move_file_shared_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", roots[0].id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let document_link = cores[1]
        .create_file("document_link", roots[1].id, FileType::Link { target: document.id })
        .unwrap();
    let result = cores[1].move_file(document_link.id, folder.id);
    assert_matches!(result, Err(UiError(MoveFileError::LinkInSharedFolder)));
}

#[test]
fn move_file_shared_link_in_folder_a() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", roots[0].id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let child_folder = cores[1]
        .create_file("child_folder", folder.id, FileType::Folder)
        .unwrap();
    let document_link = cores[1]
        .create_file("document_link", roots[1].id, FileType::Link { target: document.id })
        .unwrap();
    let result = cores[1].move_file(document_link.id, child_folder.id);
    assert_matches!(result, Err(UiError(MoveFileError::LinkInSharedFolder)));
}

#[test]
fn move_file_shared_link_in_folder_b() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", roots[0].id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let child_folder = cores[1]
        .create_file("child_folder", roots[1].id, FileType::Folder)
        .unwrap();
    cores[1]
        .create_file("document_link", child_folder.id, FileType::Link { target: document.id })
        .unwrap();
    let result = cores[1].move_file(child_folder.id, folder.id);
    assert_matches!(result, Err(UiError(MoveFileError::LinkInSharedFolder)));
}

#[test]
fn move_file_in_read_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let child_folder = cores[0]
        .create_file("folder", folder.id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let result = cores[1].move_file(document.id, child_folder.id);
    assert_matches!(result, Err(UiError(MoveFileError::InsufficientPermission)));
}

#[test]
fn move_file_in_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let child_folder = cores[0]
        .create_file("folder", folder.id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    cores[1].move_file(document.id, child_folder.id).unwrap();
}

#[test]
fn move_file_into_read_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let document = cores[1]
        .create_file("document", roots[1].id, FileType::Document)
        .unwrap();
    let result = cores[1].move_file(document.id, folder.id);
    assert_matches!(result, Err(UiError(MoveFileError::InsufficientPermission)));
}

#[test]
fn move_file_into_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let document = cores[1]
        .create_file("document", roots[1].id, FileType::Document)
        .unwrap();
    cores[1].move_file(document.id, folder.id).unwrap();
}

#[test]
fn move_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let child_folder = cores[1]
        .create_file("child_folder", roots[1].id, FileType::Folder)
        .unwrap();
    let result = cores[1].move_file(folder.id, child_folder.id);
    assert_matches!(result, Err(UiError(MoveFileError::InsufficientPermission)));
}

#[test]
fn delete_file_in_read_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let result = cores[1].delete_file(document.id);
    assert_matches!(result, Err(UiError(FileDeleteError::InsufficientPermission)));
}

#[test]
fn delete_file_in_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    let document = cores[0]
        .create_file("document", folder.id, FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    cores[1].delete_file(document.id).unwrap();
}

#[test]
fn delete_write_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder.id })
        .unwrap();
    let result = cores[1].delete_file(folder.id);
    assert_matches!(result, Err(UiError(FileDeleteError::InsufficientPermission)));
}

#[test]
fn list_metadatas_resolves_links() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder_id = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap()
        .id;
    cores[0].create_at_path("folder/file1.md").unwrap();
    cores[0].create_at_path("folder/file2.md").unwrap();
    cores[0].create_at_path("folder/file3.md").unwrap();
    cores[0].create_at_path("folder/folder2/file4.md").unwrap();
    cores[0]
        .share_file(folder_id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let link_id = cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder_id })
        .unwrap()
        .id;

    let mut files: Vec<HashMap<Uuid, File>> = vec![HashMap::new(), HashMap::new()];

    // no links
    for (index, core) in cores.iter().enumerate() {
        for file in core.list_metadatas().unwrap() {
            if !file.is_document() && !file.is_folder() {
                panic!("non document / folder file found: {:#?}", file);
            }
            files[index].insert(file.id, file);
        }
    }

    // no orphans
    for (index, core) in cores.iter().enumerate() {
        for file in core.list_metadatas().unwrap() {
            assert!(files[index].contains_key(&file.parent));
        }
    }

    // link id should be present in core1
    assert!(files[1].contains_key(&link_id));

    // Check children of links
    assert_eq!(cores[1].get_children(link_id).unwrap().len(), 4);
    assert_eq!(
        cores[1]
            .get_and_get_children_recursively(link_id)
            .unwrap()
            .len(),
        6
    );
}

#[test]
fn get_children_resolves_links() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();
    let folder_id = cores[0]
        .create_file("folder", roots[0].id, FileType::Folder)
        .unwrap()
        .id;
    cores[0].create_at_path("folder/file1.md").unwrap();
    cores[0].create_at_path("folder/file2.md").unwrap();
    cores[0].create_at_path("folder/folder2/file4.md").unwrap();
    cores[0]
        .share_file(folder_id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[1].create_at_path("file1.md").unwrap();
    cores[1].create_at_path("file2.md").unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let link_id = cores[1]
        .create_file("folder_link", roots[1].id, FileType::Link { target: folder_id })
        .unwrap()
        .id;

    // no links
    for (index, core) in cores.iter().enumerate() {
        for file in core.get_children(roots[index].id).unwrap() {
            if !file.is_document() && !file.is_folder() {
                panic!("non document / folder file found: {:#?}", file);
            }
        }
    }

    // link id should be present in core1
    assert!(cores[1]
        .get_children(roots[1].id)
        .unwrap()
        .iter()
        .map(|file| file.id)
        .contains(&link_id));

    // Check children of root
    assert_eq!(cores[1].get_children(roots[1].id).unwrap().len(), 3);

    // Check children of links
    assert_eq!(cores[1].get_children(link_id).unwrap().len(), 3);
    assert_eq!(
        cores[1]
            .get_and_get_children_recursively(link_id)
            .unwrap()
            .len(),
        5
    );
}
