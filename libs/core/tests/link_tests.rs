use lockbook_core::{Core, File};
use lockbook_shared::file::ShareMode;
use std::collections::HashSet;
use test_utils::*;
use uuid::Uuid;

fn assert_valid_list_metadatas(c: &Core) {
    let mut files: HashSet<Uuid> = HashSet::new();

    // no links
    for file in c.list_metadatas().unwrap() {
        if !file.is_document() && !file.is_folder() {
            panic!("non document/folder file in listed metadata: {:#?}", file);
        }
        files.insert(file.id);
    }
    // no orphans
    for file in c.list_metadatas().unwrap() {
        assert!(files.contains(&file.parent));
    }
}

#[test]
fn get_path_document_link() {
    let cores: Vec<Core> = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", document.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert_eq!(cores[1].get_by_path("/link").unwrap().id, document.id);
}
#[test]
fn get_path_folder_link() {
    let cores: Vec<Core> = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert_eq!(cores[1].get_by_path("/link").unwrap().id, folder.id);
}

#[test]
fn create_path_doc_under_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder.id).unwrap();

    let document = cores[1].create_at_path("link/document").unwrap();

    assert::all_ids(&cores[1], &[roots[1].id, document.id, folder.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
}

#[test]
fn create_path_folder_under_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder.id).unwrap();

    let folder1 = cores[1].create_at_path("link/folder/").unwrap();

    assert::all_ids(&cores[1], &[roots[1].id, folder1.id, folder.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/folder/"]);
}
#[test]
fn list_metadatas_link() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", document.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[document.id]);
    assert::all_recursive_children_ids(&cores[1], roots[1].id, &[roots[1].id, document.id]);
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]);
}

#[test]
fn list_metadatas_linked_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    let document = cores[0].create_at_path("folder/document").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let _link = cores[1].create_link_at_path("link", folder.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, folder.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[folder.id]);
    assert::all_children_ids(&cores[1], folder.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], folder.id, &[folder.id, document.id]);
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]);
}

#[test]
fn list_metadatas_linked_nested_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("folder/").unwrap();
    let folder2 = cores[0].create_at_path("folder/folder/").unwrap();
    let document = cores[0].create_at_path("folder/folder/document").unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder1.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, folder1.id, folder2.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/folder/", "/link/folder/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[folder1.id]);
    assert::all_children_ids(&cores[1], folder1.id, &[folder2.id]);
    assert::all_children_ids(&cores[1], folder2.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder1.id, folder2.id, document.id],
    );
    assert::all_recursive_children_ids(
        &cores[1],
        folder1.id,
        &[folder1.id, folder2.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], folder2.id, &[folder2.id, document.id]);
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]);
}

#[test]
fn list_metadatas_linked_folder_shared_from_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder2 = cores[0].create_at_path("folder/folder/").unwrap();
    let document = cores[0].create_at_path("folder/folder/document").unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder2.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, folder2.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[folder2.id]);
    assert::all_children_ids(&cores[1], folder2.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder2.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], folder2.id, &[folder2.id, document.id]);
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]);
}

#[test]
fn list_metadatas_folder_linked_into_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("folder/").unwrap();
    let document = cores[0].create_at_path("folder/document").unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let folder2 = cores[1].create_at_path("folder/").unwrap();
    cores[1]
        .create_link_at_path("folder/link", folder1.id)
        .unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, folder2.id, folder1.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/folder/", "/folder/link/", "/folder/link/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[folder2.id]);
    assert::all_children_ids(&cores[1], folder2.id, &[folder1.id]);
    assert::all_children_ids(&cores[1], folder1.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder2.id, folder1.id, document.id],
    );
    assert::all_recursive_children_ids(
        &cores[1],
        folder2.id,
        &[folder2.id, folder1.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], folder1.id, &[folder1.id, document.id]);
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]);
}

#[test]
fn list_metadatas_nested_linked_folders() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("folder/").unwrap();
    let folder2 = cores[0].create_at_path("folder/folder/").unwrap();
    let document = cores[0].create_at_path("folder/folder/document").unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link1", folder1.id).unwrap();
    cores[1].create_link_at_path("link2", folder2.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, folder1.id, folder2.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link1/", "/link2/", "/link2/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[folder1.id, folder2.id]);
    assert::all_children_ids(&cores[1], folder2.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder1.id, folder2.id, document.id],
    );
    assert::all_recursive_children_ids(
        &cores[1],
        folder1.id,
        &[folder1.id, folder2.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], folder2.id, &[folder2.id, document.id]);
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]);
}

#[test]
fn inconsistent_share_finalization() {
    let cores: Vec<Core> =
        vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[2].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    cores[2].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder.id).unwrap();

    let file_single_finalization = cores[1].get_file_by_id(folder.id).unwrap();

    let files_all_finalization = cores[1]
        .get_and_get_children_recursively(roots[1].id)
        .unwrap();

    let file_all_finalization: &File = files_all_finalization
        .iter()
        .find(|f| f.id == folder.id)
        .unwrap();

    assert_eq!(file_all_finalization.shares, file_single_finalization.shares);
}

#[test]
fn link_resolving() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let _roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("lockbook/").unwrap();
    let file = cores[0]
        .create_file("test.md", folder.id, lockbook_core::FileType::Document)
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].create_link_at_path("link", folder.id).unwrap();

    assert_eq!(cores[1].get_file_by_id(file.id).unwrap().parent, folder.id);
}
