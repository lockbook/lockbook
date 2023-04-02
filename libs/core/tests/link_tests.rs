use lockbook_core::Core;
use lockbook_shared::file::ShareMode;
use rand::Rng;
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

    let link = cores[1].create_link_at_path("link", document.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, link.id]);
    assert::all_paths(&cores[1], &["/", "/link"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[link.id]);
    assert::all_recursive_children_ids(&cores[1], roots[1].id, &[roots[1].id, link.id]);
    assert::all_recursive_children_ids(&cores[1], link.id, &[link.id]);
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

    let link = cores[1].create_link_at_path("link", folder.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, link.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[link.id]);
    assert::all_children_ids(&cores[1], link.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, link.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], link.id, &[link.id, document.id]);
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

    let link = cores[1].create_link_at_path("link", folder1.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, link.id, folder2.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/folder/", "/link/folder/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[link.id]);
    assert::all_children_ids(&cores[1], link.id, &[folder2.id]);
    assert::all_children_ids(&cores[1], folder2.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, link.id, folder2.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], link.id, &[link.id, folder2.id, document.id]);
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

    let link = cores[1].create_link_at_path("link", folder2.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, link.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[link.id]);
    assert::all_children_ids(&cores[1], link.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, link.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], link.id, &[link.id, document.id]);
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
    let link = cores[1]
        .create_link_at_path("folder/link", folder1.id)
        .unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, folder2.id, link.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/folder/", "/folder/link/", "/folder/link/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[folder2.id]);
    assert::all_children_ids(&cores[1], folder2.id, &[link.id]);
    assert::all_children_ids(&cores[1], link.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder2.id, link.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], folder2.id, &[folder2.id, link.id, document.id]);
    assert::all_recursive_children_ids(&cores[1], link.id, &[link.id, document.id]);
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

    let link1 = cores[1].create_link_at_path("link1", folder1.id).unwrap();
    let link2 = cores[1].create_link_at_path("link2", folder2.id).unwrap();

    assert_valid_list_metadatas(&cores[0]);
    assert_valid_list_metadatas(&cores[1]);
    assert::all_ids(&cores[1], &[roots[1].id, link1.id, link2.id, document.id]);
    assert::all_paths(&cores[1], &["/", "/link1/", "/link2/", "/link2/document"]);
    assert::all_children_ids(&cores[1], roots[1].id, &[link1.id, link2.id]);
    assert::all_children_ids(&cores[1], link2.id, &[document.id]);
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, link1.id, link2.id, document.id],
    );
    assert::all_recursive_children_ids(&cores[1], link1.id, &[link1.id, document.id]); // todo: is this correct?
    assert::all_recursive_children_ids(&cores[1], link2.id, &[link2.id, document.id]);
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]);
}
