use lockbook_core::Error::UiError;
use lockbook_core::{
    CreateFileAtPathError, CreateLinkAtPathError, DeletePendingShareError, Error, ShareFileError,
    WriteToDocumentError,
};
use lockbook_shared::file::ShareMode;
use lockbook_shared::file_metadata::FileType;
use test_utils::*;
use uuid::Uuid;

#[test]
fn write_document_read_share() {
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
    let result = cores[1].write_document(document0.id, b"document content");
    assert_matches!(result, Err(Error::UiError(WriteToDocumentError::InsufficientPermission)));
}

#[test]
fn write_document_write_share() {
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
        .share_file(document0.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document0.id, b"document content")
        .unwrap();
    assert_eq!(cores[1].read_document(document0.id).unwrap(), b"document content");
    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();
    assert_eq!(cores[0].read_document(document0.id).unwrap(), b"document content");
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
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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

    assert_matches!(result, Err(UiError(CreateFileAtPathError::InsufficientPermission)));
}

#[test]
#[ignore]
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

    assert_matches!(result, Err(_));
}

#[test]
#[ignore]
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
#[ignore]
fn create_link_at_path_target_nonexistent() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let result = core.create_link_at_path(&format!("{}/link", &account.username), Uuid::new_v4());
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::LinkTargetNonexistent)));
}

#[test]
#[ignore]
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

    let result = cores[1].create_link_at_path(
        &format!("{}/folder_link/document", &accounts[1].username),
        document0.id,
    );
    assert_matches!(result, Err(UiError(CreateLinkAtPathError::LinkInSharedFolder)));
}

#[test]
#[ignore]
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

// #[test]
// fn apply_create() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();

//     files::apply_create(
//         &Owner(account.public_key()),
//         &[root.clone()].to_map(),
//         FileType::Document,
//         root.id,
//         "document",
//     )
//     .unwrap();
// }

// #[test]
// fn apply_create_parent_does_not_exist() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();

//     let result = files::apply_create(
//         &Owner(account.public_key()),
//         &[root.clone()].to_map(),
//         FileType::Document,
//         Uuid::new_v4(),
//         "document",
//     );
//     assert_eq!(result, Err(CoreError::FileParentNonexistent));
// }

// #[test]
// fn apply_create_path_taken() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let same_path_file =
//         files::create(FileType::Document, root.id, "document", &account.public_key());

//     let result = files::apply_create(
//         &Owner(account.public_key()),
//         &[root.clone(), same_path_file].to_map(),
//         FileType::Document,
//         root.id,
//         "document",
//     );
//     assert_eq!(result, Err(CoreError::PathTaken));
// }

// #[test]
// fn apply_create_link_target_nonexistent() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();

//     let result = files::apply_create(
//         &Owner(account.public_key()),
//         &[root.clone()].to_map(),
//         FileType::Link { linked_file: Uuid::new_v4() },
//         root.id,
//         "link",
//     );
//     assert_eq!(result, Err(CoreError::LinkTargetNonexistent));
// }

// #[test]
// fn apply_create_link_target_owned() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let document = files::create(FileType::Document, root.id, "document", &account.public_key());

//     let result = files::apply_create(
//         &Owner(account.public_key()),
//         &[root.clone(), document.clone()].to_map(),
//         FileType::Link { linked_file: document.id },
//         root.id,
//         "link",
//     );
//     assert_eq!(result, Err(CoreError::LinkTargetIsOwned));
// }

// #[test]
// fn apply_create_shared_link() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut linked_shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     linked_shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read,
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });

//     let result = files::apply_create(
//         &Owner(account.public_key()),
//         &[linked_shared_folder.clone(), root.clone()].to_map(),
//         FileType::Link { linked_file: linked_shared_folder.id },
//         linked_shared_folder.id,
//         "link",
//     );
//     assert_eq!(result, Err(CoreError::LinkInSharedFolder));
// }

// #[test]
// fn apply_create_duplicate_link() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut linked_shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     linked_shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read,
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let duplicate_link = files::create(
//         FileType::Link { linked_file: linked_shared_folder.id },
//         root.id,
//         "duplicate_link",
//         &account.public_key(),
//     );
//     let result = files::apply_create(
//         &Owner(account.public_key()),
//         &[duplicate_link, linked_shared_folder.clone(), root.clone()].to_map(),
//         FileType::Link { linked_file: linked_shared_folder.id },
//         root.id,
//         "link",
//     );
//     assert_eq!(result, Err(CoreError::MultipleLinksToSameFile));
// }

// #[test]
// fn apply_create_in_read_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read, // note: read access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let result = files::apply_create(
//         &Owner(account.public_key()),
//         &[shared_folder.clone(), root.clone()].to_map(),
//         FileType::Document,
//         shared_folder.id,
//         "document",
//     );
//     assert_eq!(result, Err(CoreError::InsufficientPermission));
// }

// #[test]
// fn apply_create_in_write_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Write, // note: write access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     files::apply_create(
//         &Owner(account.public_key()),
//         &[shared_folder.clone(), root.clone()].to_map(),
//         FileType::Document,
//         shared_folder.id,
//         "document",
//     )
//     .unwrap();
// }

// #[test]
// fn apply_rename_in_read_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read, // note: read access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     let result = files::apply_rename(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder.clone(), shared_folder.clone(), root].to_map(),
//         file_in_shared_folder.id,
//         "document2",
//     );
//     assert_eq!(result, Err(CoreError::InsufficientPermission));
// }

// #[test]
// fn apply_rename_in_write_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Write, // note: write access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     files::apply_rename(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder.clone(), shared_folder.clone(), root].to_map(),
//         file_in_shared_folder.id,
//         "document2",
//     )
//     .unwrap();
// }

// #[test]
// fn apply_rename_to_write_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Write, // note: write access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     let result = files::apply_rename(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder, shared_folder.clone(), root].to_map(),
//         shared_folder.id,
//         "linked_shared_folder2",
//     );
//     assert_eq!(result, Err(CoreError::InsufficientPermission));
// }

// #[test]
// fn apply_move_shared_link() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut linked_shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     linked_shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read,
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let link = files::create(
//         FileType::Link { linked_file: linked_shared_folder.id },
//         root.id,
//         "link",
//         &account.public_key(),
//     );

//     let result = files::apply_move(
//         &Owner(account.public_key()),
//         &[root, linked_shared_folder.clone(), link.clone()].to_map(),
//         link.id,
//         linked_shared_folder.id,
//     );
//     assert_eq!(result, Err(CoreError::LinkInSharedFolder));
// }

// #[test]
// fn apply_move_shared_link_in_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut linked_shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     linked_shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read,
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
//     let link = files::create(
//         FileType::Link { linked_file: linked_shared_folder.id },
//         folder.id,
//         "link",
//         &account.public_key(),
//     );

//     let result = files::apply_move(
//         &Owner(account.public_key()),
//         &[root, linked_shared_folder.clone(), folder.clone(), link].to_map(),
//         folder.id,
//         linked_shared_folder.id,
//     );
//     assert_eq!(result, Err(CoreError::LinkInSharedFolder));
// }

// #[test]
// fn apply_move_in_read_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read, // note: read access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     let result = files::apply_move(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder.clone(), shared_folder.clone(), root].to_map(),
//         file_in_shared_folder.id,
//         shared_folder.id,
//     );
//     assert_eq!(result, Err(CoreError::InsufficientPermission));
// }

// #[test]
// fn apply_move_in_write_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Write, // note: write access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     files::apply_move(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder.clone(), shared_folder.clone(), root].to_map(),
//         file_in_shared_folder.id,
//         shared_folder.id,
//     )
//     .unwrap();
// }

// #[test]
// fn apply_move_to_write_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Write, // note: write access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     let result = files::apply_move(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder, shared_folder.clone(), root].to_map(),
//         shared_folder.id,
//         shared_folder.id,
//     );
//     assert_eq!(result, Err(CoreError::InsufficientPermission));
// }

// #[test]
// fn apply_delete_in_read_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Read, // note: read access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     let result = files::apply_delete(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder.clone(), shared_folder, root].to_map(),
//         file_in_shared_folder.id,
//     );
//     assert_eq!(result, Err(CoreError::InsufficientPermission));
// }

// #[test]
// fn apply_delete_in_write_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Write, // note: write access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     files::apply_delete(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder.clone(), shared_folder, root].to_map(),
//         file_in_shared_folder.id,
//     )
//     .unwrap();
// }

// #[test]
// fn apply_delete_to_write_shared_folder() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let sharer_public_key = test_core_with_account().get_account().unwrap().public_key();
//     let mut shared_folder =
//         files::create(FileType::Folder, Uuid::new_v4(), "linked_shared_folder", &sharer_public_key);
//     shared_folder.shares.push(UserAccessInfo {
//         mode: UserAccessMode::Write, // note: write access
//         encrypted_by_username: String::from("sharer_username"),
//         encrypted_by_public_key: sharer_public_key,
//         encrypted_for_username: account.username.clone(),
//         encrypted_for_public_key: account.public_key(),
//         access_key: AESEncrypted::<[u8; 32]> {
//             value: Default::default(),
//             nonce: Default::default(),
//             _t: Default::default(),
//         },
//         file_name: SecretFileName {
//             encrypted_value: AESEncrypted::<String> {
//                 value: Default::default(),
//                 nonce: Default::default(),
//                 _t: Default::default(),
//             },
//             hmac: Default::default(),
//         },
//         deleted: false,
//     });
//     let file_in_shared_folder =
//         files::create(FileType::Document, shared_folder.id, "document", &sharer_public_key);

//     let result = files::apply_delete(
//         &Owner(account.public_key()),
//         &[file_in_shared_folder, shared_folder.clone(), root].to_map(),
//         shared_folder.id,
//     );
//     assert_eq!(result, Err(CoreError::InsufficientPermission));
// }

// /*  ---------------------------------------------------------------------------------------------------------------
// Tests that setup one device each on two accounts, share a file from one to the other, then sync both
// ---------------------------------------------------------------------------------------------------------------  */
// #[test]
// fn pending_share() {
//     for mut ops in [
//         // new_file
//         vec![
//             Client { account_num: 0, client_num: 0 },
//             Client { account_num: 1, client_num: 1 },
//             Create { client_num: 0, path: "document" },
//             Share {
//                 client_num: 0,
//                 sharee_account_num: 1,
//                 share_mode: ShareMode::Read,
//                 path: "document",
//             },
//             Sync { client_num: 0 },
//             Sync { client_num: 1 },
//             Custom {
//                 f: &|dbs| {
//                     let db = &dbs[1];
//                     test_utils::assert_all_pending_shares(db, &["document"]);
//                 },
//             },
//         ],
//         // new_files
//         vec![
//             Client { account_num: 0, client_num: 0 },
//             Client { account_num: 1, client_num: 1 },
//             Create { client_num: 0, path: "a/b/c/d" },
//             Create { client_num: 0, path: "e/f/g/h" },
//             Share { client_num: 0, sharee_account_num: 1, share_mode: ShareMode::Read, path: "a" },
//             Share { client_num: 0, sharee_account_num: 1, share_mode: ShareMode::Read, path: "e" },
//             Sync { client_num: 0 },
//             Sync { client_num: 1 },
//             Custom {
//                 f: &|dbs| {
//                     let db = &dbs[1];
//                     test_utils::assert_all_pending_shares(db, &["a", "e"]);
//                 },
//             },
//         ],
//         // edited_document
//         vec![
//             Client { account_num: 0, client_num: 0 },
//             Client { account_num: 1, client_num: 1 },
//             Create { client_num: 0, path: "document" },
//             Share {
//                 client_num: 0,
//                 sharee_account_num: 1,
//                 share_mode: ShareMode::Read,
//                 path: "document",
//             },
//             Edit { client_num: 0, path: "document", content: b"document content" },
//             Sync { client_num: 0 },
//             Sync { client_num: 1 },
//             Custom {
//                 f: &|dbs| {
//                     let db = &dbs[1];
//                     test_utils::assert_all_pending_shares(db, &["document"]);
//                 },
//             },
//         ],
//     ] {
//         ops.push(Custom {
//             f: &|dbs| {
//                 for db in dbs {
//                     db.validate().unwrap();
//                     test_utils::assert_deleted_files_pruned(db);
//                     let root = &db.get_root().unwrap();
//                     test_utils::assert_local_work_paths(db, root, &[]);
//                     test_utils::assert_server_work_paths(db, root, &[]);
//                 }
//                 let db = &dbs[1];
//                 let root = &db.get_root().unwrap();
//                 test_utils::assert_all_paths(db, root, &[""]);
//                 test_utils::assert_all_document_contents(db, root, &[]);
//             },
//         });
//         test_utils::run(&ops);
//     }
// }

// /*  ---------------------------------------------------------------------------------------------------------------
// Tests that setup one device each on two accounts, share a file from one to the other, sync both, then accept
// ---------------------------------------------------------------------------------------------------------------  */
// #[test]
// fn share() {
//     for mut ops in [
//         // new_file
//         vec![
//             Client { account_num: 0, client_num: 0 },
//             Client { account_num: 1, client_num: 1 },
//             Create { client_num: 0, path: "document" },
//             Share {
//                 client_num: 0,
//                 sharee_account_num: 1,
//                 share_mode: ShareMode::Read,
//                 path: "document",
//             },
//             Sync { client_num: 0 },
//             Sync { client_num: 1 },
//             Custom {
//                 f: &|dbs| {
//                     let db = &dbs[1];
//                     let root = &db.get_root().unwrap();
//                     let shares = db.get_pending_shares().unwrap();
//                     let id = shares[0].id;
//                     db.create_link_at_path(&test_utils::path(db, "link"), id)
//                         .unwrap();

//                     test_utils::assert_all_paths(db, root, &["", "link"]);
//                     test_utils::assert_all_pending_shares(db, &[]);
//                 },
//             },
//             Sync { client_num: 1 },
//         ],
//         // new_files
//         vec![
//             Client { account_num: 0, client_num: 0 },
//             Client { account_num: 1, client_num: 1 },
//             Create { client_num: 0, path: "a/b/c/d" },
//             Create { client_num: 0, path: "e/f/g/h" },
//             Share { client_num: 0, sharee_account_num: 1, share_mode: ShareMode::Read, path: "a" },
//             Share { client_num: 0, sharee_account_num: 1, share_mode: ShareMode::Read, path: "e" },
//             Sync { client_num: 0 },
//             Sync { client_num: 1 },
//             Custom {
//                 f: &|dbs| {
//                     let db = &dbs[1];
//                     let root = &db.get_root().unwrap();
//                     let shares = db.get_pending_shares().unwrap();
//                     let (id_a, id_e) = if shares[0].decrypted_name == "a" {
//                         (shares[0].id, shares[1].id)
//                     } else {
//                         (shares[1].id, shares[0].id)
//                     };
//                     db.create_link_at_path(&test_utils::path(db, "link_a"), id_a)
//                         .unwrap();
//                     db.create_link_at_path(&test_utils::path(db, "link_e"), id_e)
//                         .unwrap();

//                     test_utils::assert_all_paths(db, root, &["", "link_a", "link_e"]);
//                     test_utils::assert_all_pending_shares(db, &[]);
//                 },
//             },
//             Sync { client_num: 1 },
//         ],
//         // edited_document
//         vec![
//             Client { account_num: 0, client_num: 0 },
//             Client { account_num: 1, client_num: 1 },
//             Create { client_num: 0, path: "document" },
//             Share {
//                 client_num: 0,
//                 sharee_account_num: 1,
//                 share_mode: ShareMode::Read,
//                 path: "document",
//             },
//             Edit { client_num: 0, path: "document", content: b"document content" },
//             Sync { client_num: 0 },
//             Sync { client_num: 1 },
//             Custom {
//                 f: &|dbs| {
//                     let db = &dbs[1];
//                     let root = &db.get_root().unwrap();
//                     let shares = db.get_pending_shares().unwrap();
//                     let id = shares[0].id;
//                     db.create_link_at_path(&test_utils::path(db, "link"), id)
//                         .unwrap();

//                     test_utils::assert_all_paths(db, root, &["", "link"]);
//                     test_utils::assert_all_pending_shares(db, &[]);
//                 },
//             },
//             Sync { client_num: 1 },
//         ],
//     ] {
//         ops.push(Custom {
//             f: &|dbs| {
//                 for db in dbs {
//                     db.validate().unwrap();
//                     test_utils::assert_deleted_files_pruned(db);
//                     let root = &db.get_root().unwrap();
//                     test_utils::assert_local_work_paths(db, root, &[]);
//                     test_utils::assert_server_work_paths(db, root, &[]);
//                 }
//                 let db = &dbs[1];
//                 test_utils::assert_all_pending_shares(db, &[]);
//             },
//         });
//         test_utils::run(&ops);
//     }
// }
