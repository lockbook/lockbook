use lb_rs::Lb;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::file_metadata::FileType;
use std::collections::HashSet;
use test_utils::*;
use uuid::Uuid;

async fn assert_valid_list_metadatas(c: &Lb) {
    let mut files: HashSet<Uuid> = HashSet::new();

    // no links
    for file in c.list_metadatas().await.unwrap() {
        if !file.is_document() && !file.is_folder() {
            panic!("non document/folder file in listed metadata: {file:#?}");
        }
        files.insert(file.id);
    }
    // no orphans
    for file in c.list_metadatas().await.unwrap() {
        assert!(files.contains(&file.parent));
    }
}

#[tokio::test]
async fn get_path_document_link() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", document.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert_eq!(cores[1].get_by_path("/link").await.unwrap().id, document.id);
}

#[tokio::test]
async fn get_path_folder_link() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert_eq!(cores[1].get_by_path("/link").await.unwrap().id, folder.id);
}

#[tokio::test]
async fn create_path_doc_under_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    let document = cores[1].create_at_path("link/document").await.unwrap();

    assert::all_ids(&cores[1], &[roots[1].id, document.id, folder.id]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]).await;
}

#[tokio::test]
async fn create_path_folder_under_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    let folder1 = cores[1].create_at_path("link/folder/").await.unwrap();

    assert::all_ids(&cores[1], &[roots[1].id, folder1.id, folder.id]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/folder/"]).await;
}

#[tokio::test]
async fn list_metadatas_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document = cores[0].create_at_path("document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", document.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert::all_ids(&cores[1], &[roots[1].id, document.id]).await;
    assert::all_paths(&cores[1], &["/", "/link"]).await;
    assert::all_children_ids(&cores[1], &roots[1].id, &[document.id]).await;
    assert::all_recursive_children_ids(&cores[1], roots[1].id, &[roots[1].id, document.id]).await;
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]).await;
}

#[tokio::test]
async fn list_metadatas_linked_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    let document = cores[0].create_at_path("folder/document").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let _link = cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert::all_ids(&cores[1], &[roots[1].id, folder.id, document.id]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]).await;
    assert::all_children_ids(&cores[1], &roots[1].id, &[folder.id]).await;
    assert::all_children_ids(&cores[1], &folder.id, &[document.id]).await;
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(&cores[1], folder.id, &[folder.id, document.id]).await;
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]).await;
}

#[tokio::test]
async fn list_metadatas_linked_nested_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder1 = cores[0].create_at_path("folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder1.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert::all_ids(&cores[1], &[roots[1].id, folder1.id, folder2.id, document.id]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/folder/", "/link/folder/document"]).await;
    assert::all_children_ids(&cores[1], &roots[1].id, &[folder1.id]).await;
    assert::all_children_ids(&cores[1], &folder1.id, &[folder2.id]).await;
    assert::all_children_ids(&cores[1], &folder2.id, &[document.id]).await;
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder1.id, folder2.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(
        &cores[1],
        folder1.id,
        &[folder1.id, folder2.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(&cores[1], folder2.id, &[folder2.id, document.id]).await;
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]).await;
}

#[tokio::test]
async fn list_metadatas_linked_folder_shared_from_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder2 = cores[0].create_at_path("folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder2.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert::all_ids(&cores[1], &[roots[1].id, folder2.id, document.id]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]).await;
    assert::all_children_ids(&cores[1], &roots[1].id, &[folder2.id]).await;
    assert::all_children_ids(&cores[1], &folder2.id, &[document.id]).await;
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder2.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(&cores[1], folder2.id, &[folder2.id, document.id]).await;
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]).await;
}

#[tokio::test]
async fn list_metadatas_folder_linked_into_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder1 = cores[0].create_at_path("folder/").await.unwrap();
    let document = cores[0].create_at_path("folder/document").await.unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let folder2 = cores[1].create_at_path("folder/").await.unwrap();
    cores[1]
        .create_link_at_path("folder/link", folder1.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert::all_ids(&cores[1], &[roots[1].id, folder2.id, folder1.id, document.id]).await;
    assert::all_paths(&cores[1], &["/", "/folder/", "/folder/link/", "/folder/link/document"])
        .await;
    assert::all_children_ids(&cores[1], &roots[1].id, &[folder2.id]).await;
    assert::all_children_ids(&cores[1], &folder2.id, &[folder1.id]).await;
    assert::all_children_ids(&cores[1], &folder1.id, &[document.id]).await;
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder2.id, folder1.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(
        &cores[1],
        folder2.id,
        &[folder2.id, folder1.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(&cores[1], folder1.id, &[folder1.id, document.id]).await;
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]).await;
}

#[tokio::test]
async fn list_metadatas_nested_linked_folders() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder1 = cores[0].create_at_path("folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link1", folder1.id)
        .await
        .unwrap();
    cores[1]
        .create_link_at_path("link2", folder2.id)
        .await
        .unwrap();

    assert_valid_list_metadatas(&cores[0]).await;
    assert_valid_list_metadatas(&cores[1]).await;
    assert::all_ids(&cores[1], &[roots[1].id, folder1.id, folder2.id, document.id]).await;
    assert::all_paths(&cores[1], &["/", "/link1/", "/link2/", "/link2/document"]).await;
    assert::all_children_ids(&cores[1], &roots[1].id, &[folder1.id, folder2.id]).await;
    assert::all_children_ids(&cores[1], &folder2.id, &[document.id]).await;
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder1.id, folder2.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(
        &cores[1],
        folder1.id,
        &[folder1.id, folder2.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(&cores[1], folder2.id, &[folder2.id, document.id]).await;
    assert::all_recursive_children_ids(&cores[1], document.id, &[document.id]).await;
}

#[tokio::test]
async fn inconsistent_share_finalization() {
    let cores: Vec<Lb> = vec![
        test_core_with_account().await,
        test_core_with_account().await,
        test_core_with_account().await,
    ];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [
        cores[0].root().await.unwrap(),
        cores[1].root().await.unwrap(),
        cores[2].root().await.unwrap(),
    ];

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[2].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[2].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    let file_single_finalization = cores[1].get_file_by_id(folder.id).await.unwrap();

    let files_all_finalization = cores[1]
        .get_and_get_children_recursively(&roots[1].id)
        .await
        .unwrap();

    let file_all_finalization: &File = files_all_finalization
        .iter()
        .find(|f| f.id == folder.id)
        .unwrap();

    assert_eq!(file_all_finalization.shares, file_single_finalization.shares);
}

#[tokio::test]
async fn link_resolving() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("lockbook/").await.unwrap();
    let file = cores[0]
        .create_file("test.md", &folder.id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("link", folder.id)
        .await
        .unwrap();

    assert_eq!(cores[1].get_file_by_id(file.id).await.unwrap().parent, folder.id);
}
