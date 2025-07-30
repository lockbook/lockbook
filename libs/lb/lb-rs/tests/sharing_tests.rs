use lb_rs::Lb;
use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::FileType;
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn shares_finalized() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    assert_eq!(
        cores[0].get_file_by_id(document.id).await.unwrap().shares[0].shared_with,
        accounts[1].username
    );
}

#[tokio::test]
async fn shares_finalized_unsynced_share() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();

    assert_eq!(
        cores[0].get_file_by_id(document.id).await.unwrap().shares[0].shared_with,
        accounts[1].username
    );
}

#[tokio::test]
async fn write_document_read_share() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();

    let result = cores[1]
        .write_document(document.id, b"document content")
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn write_document_in_read_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("/folder/").await.unwrap();
    let document = cores[0].create_at_path("/folder/document").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();

    let result = cores[1]
        .write_document(document.id, b"document content")
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn write_document_write_share() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_document_in_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("/folder/").await.unwrap();
    let document = cores[0].create_at_path("/folder/document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_document_in_write_shared_folder_in_read_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_document_in_read_shared_folder_in_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(folder2.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_document_rejected_share() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1].reject_share(&document.id).await.unwrap();

    let result = cores[1]
        .write_document(document.id, b"document content by sharee")
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn write_document_in_shared_folder_in_rejected_share_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
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
    cores[1].reject_share(&folder2.id).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_document_in_rejected_shared_folder_in_share_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
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
    cores[1].reject_share(&folder1.id).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[1].get_file_by_id(document.id).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_document_in_rejected_shared_folder_in_rejected_share_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
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
    cores[1].reject_share(&folder1.id).await.unwrap();
    cores[1].reject_share(&folder2.id).await.unwrap();

    let result = cores[1]
        .write_document(document.id, b"document content by sharee")
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn write_link_by_sharee() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document1 = cores[0].create_at_path("/document1").await.unwrap();

    cores[0]
        .share_file(document1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let link = cores[1]
        .create_link_at_path("/link1", document1.id)
        .await
        .unwrap();
    cores[1]
        .write_document(link.id, b"document content by sharee")
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    assert_eq!(
        cores[0].read_document(document1.id, false).await.unwrap(),
        b"document content by sharee"
    );
    assert_eq!(
        cores[1].read_document(document1.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_target_by_sharee() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document1 = cores[0].create_at_path("/document1").await.unwrap();

    cores[0]
        .share_file(document1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("/link1", document1.id)
        .await
        .unwrap();
    cores[1]
        .write_document(document1.id, b"document content by sharee")
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    assert_eq!(
        cores[0].read_document(document1.id, false).await.unwrap(),
        b"document content by sharee"
    );
    assert_eq!(
        cores[1].read_document(document1.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn create_document_in_link_folder_by_sharee() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();

    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let link = cores[1]
        .create_link_at_path("/link1", folder1.id)
        .await
        .unwrap();
    let result = cores[1]
        .create_file("document1", &link.id, FileType::Document)
        .await
        .unwrap_err();

    assert_matches!(
        result.kind,
        LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(_))
    );
}

#[tokio::test]
async fn create_document_in_link_folder_by_sharer() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();

    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let link = cores[1]
        .create_link_at_path("/link1", folder1.id)
        .await
        .unwrap();
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    let result = cores[0]
        .create_file("document1", &link.id, FileType::Document)
        .await
        .unwrap_err();

    assert_eq!(result.kind, LbErrKind::FileNonexistent);
}

#[tokio::test]
async fn create_document_in_target_folder_by_sharee() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();

    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("/link1", folder1.id)
        .await
        .unwrap();
    cores[1]
        .create_file("document1", &folder1.id, FileType::Document)
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    assert::all_paths(&cores[1], &["/", "/link1/", "/link1/document1"]).await;
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document1"]).await;
}

#[tokio::test]
async fn create_document_in_target_folder_by_sharer() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();

    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("/link1", folder1.id)
        .await
        .unwrap();
    cores[0]
        .create_file("document1", &folder1.id, FileType::Document)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    assert::all_paths(&cores[1], &["/", "/link1/", "/link1/document1"]).await;
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document1"]).await;
}

#[tokio::test]
async fn get_link_target_children_by_sharee() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(folder1.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("/link1", folder1.id)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    assert::all_recursive_children_ids(
        &cores[1],
        folder1.id,
        &[folder1.id, folder2.id, document.id],
    )
    .await;
    assert::all_recursive_children_ids(
        &cores[1],
        roots[1].id,
        &[roots[1].id, folder1.id, folder2.id, document.id],
    )
    .await;

    assert::all_children_ids(&cores[1], &folder1.id, &[folder2.id]).await;
    assert::all_children_ids(&cores[1], &folder2.id, &[document.id]).await;
    assert::all_children_ids(&cores[1], &roots[1].id, &[folder1.id]).await;
}

#[tokio::test]
async fn linked_nested_shared_folders_distinct_path_changes_when_closest_link_deleted() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder1 = cores[0].create_at_path("/folder/").await.unwrap();
    let folder2 = cores[0].create_at_path("/folder/folder/").await.unwrap();
    let document = cores[0]
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
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
        .create_link_at_path("/link1", folder1.id)
        .await
        .unwrap();
    let link2 = cores[1]
        .create_link_at_path("/link2", folder2.id)
        .await
        .unwrap();

    assert::all_paths(&cores[1], &["/", "/link1/", "/link2/", "/link2/document"]).await;
    cores[1].get_by_path("/link2/document").await.unwrap();
    cores[1]
        .get_by_path("/link1/folder/document")
        .await
        .unwrap_err();

    cores[1].delete(&link2.id).await.unwrap();

    assert::all_paths(&cores[1], &["/", "/link1/", "/link1/folder/", "/link1/folder/document"])
        .await;
    cores[1].get_by_path("/link2/document").await.unwrap_err();
    cores[1]
        .get_by_path("/link1/folder/document")
        .await
        .unwrap();
}

#[tokio::test]
async fn write_document_write_share_by_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();
    cores[1]
        .write_document(link.id, b"document content by sharee")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn write_document_deleted_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();
    cores[1]
        .write_document(link.id, b"document content by sharee")
        .await
        .unwrap();
    cores[1].delete(&link.id).await.unwrap();
    cores[1]
        .write_document(link.id, b"document content by sharee 2")
        .await
        .unwrap();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee 2"
    );
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharee 2"
    );
}

#[tokio::test]
async fn write_document_link_deleted_when_share_rejected() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();
    cores[1]
        .write_document(link.id, b"document content by sharee")
        .await
        .unwrap();
    cores[1].get_file_by_id(link.id).await.unwrap();
    cores[1].reject_share(&document.id).await.unwrap();
    cores[1].get_file_by_id(link.id).await.unwrap_err();

    assert_eq!(
        cores[1].read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
    cores[1].sync(None).await.unwrap();
    cores[1].get_file_by_id(document.id).await.unwrap_err();
    cores[0].sync(None).await.unwrap();
    assert_eq!(
        cores[0].read_document(document.id, false).await.unwrap(),
        b"document content by sharer"
    );
}

#[tokio::test]
async fn share_file_root() {
    let core = test_core_with_account().await;
    let sharee_core = test_core_with_account().await;
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.root().await.unwrap();

    let result = core
        .share_file(root.id, &sharee_account.username, ShareMode::Read)
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::RootModificationInvalid);
}

#[tokio::test]
async fn share_file_nonexistent() {
    let core = test_core_with_account().await;
    let sharee_core = test_core_with_account().await;
    let sharee_account = &sharee_core.get_account().unwrap();

    let result = core
        .share_file(Uuid::new_v4(), &sharee_account.username, ShareMode::Read)
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::FileNonexistent);
}

#[tokio::test]
async fn share_file_in_shared_folder() {
    let core = test_core_with_account().await;
    let sharee_core = test_core_with_account().await;
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.root().await.unwrap();
    let outer_folder = core
        .create_file("outer_folder", &root.id, FileType::Folder)
        .await
        .unwrap();
    let inner_folder = core
        .create_file("inner_folder", &outer_folder.id, FileType::Folder)
        .await
        .unwrap();
    core.share_file(outer_folder.id, &sharee_account.username, ShareMode::Read)
        .await
        .unwrap();

    core.share_file(inner_folder.id, &sharee_account.username, ShareMode::Read)
        .await
        .unwrap();
}

#[tokio::test]
async fn delete_nonexistent_share() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let document = core
        .create_file("document", &root.id, FileType::Document)
        .await
        .unwrap();

    let result = core.reject_share(&document.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::ShareNonexistent);
}

#[tokio::test]
async fn test_deleted_share() {
    let cores: Vec<Lb> = vec![test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let doc = cores[0].create_at_path("asrar.md").await.unwrap();
    cores[0]
        .share_file(doc.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1]
        .create_link_at_path("/asrar.md", doc.id)
        .await
        .unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1].reject_share(&doc.id).await.unwrap();

    cores[1].sync(None).await.unwrap();

    let fresh = test_core_from(&cores[1]).await;
    fresh.sync(None).await.unwrap();
    let root = fresh.root().await.unwrap().id;
    fresh.get_children(&root).await.unwrap();
}

#[tokio::test]
async fn share_file_duplicate_original_deleted() {
    let core = test_core_with_account().await;
    let sharee_core = test_core_with_account().await;
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.root().await.unwrap();
    let document = core
        .create_file("document", &root.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Write)
        .await
        .unwrap();
    core.sync(None).await.unwrap();

    sharee_core.sync(None).await.unwrap();
    sharee_core.reject_share(&document.id).await.unwrap();
    sharee_core.sync(None).await.unwrap();

    core.sync(None).await.unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Write)
        .await
        .unwrap();
    core.sync(None).await.unwrap();

    sharee_core.sync(None).await.unwrap();
    sharee_core
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();
    sharee_core.sync(None).await.unwrap();

    core.sync(None).await.unwrap();
    assert_eq!(
        core.read_document(document.id, false).await.unwrap(),
        b"document content by sharee"
    );
}

#[tokio::test]
async fn share_file_duplicate() {
    let core = test_core_with_account().await;
    let sharee_core = test_core_with_account().await;
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.root().await.unwrap();
    let document = core
        .create_file("document", &root.id, FileType::Document)
        .await
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Read)
        .await
        .unwrap();

    let result = core
        .share_file(document.id, &sharee_account.username, ShareMode::Read)
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::ShareAlreadyExists);
}

#[tokio::test]
async fn share_file_duplicate_new_mode() {
    let core = test_core_with_account().await;
    let sharee_core = test_core_with_account().await;
    let sharee_account = &sharee_core.get_account().unwrap();
    let root = core.root().await.unwrap();
    let document = core
        .create_file("document", &root.id, FileType::Document)
        .await
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Read)
        .await
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Write)
        .await
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Read)
        .await
        .unwrap();
    core.share_file(document.id, &sharee_account.username, ShareMode::Write)
        .await
        .unwrap();
}

#[tokio::test]
async fn share_folder_with_link_inside() {
    let cores = [
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

    let folder0 = cores[0]
        .create_file("folder0", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let folder1 = cores[1]
        .create_file("folder1", &roots[1].id, FileType::Folder)
        .await
        .unwrap();
    cores[1]
        .create_file("link", &folder1.id, FileType::Link { target: folder0.id })
        .await
        .unwrap();

    let result = cores[1]
        .share_file(folder1.id, &accounts[2].username, ShareMode::Read)
        .await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::SharedLink { .. })
    );
}

#[tokio::test]
async fn share_unowned_file_read() {
    let cores = [
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

    let folder0 = cores[0]
        .create_file("folder0", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[1]
        .share_file(folder0.id, &accounts[2].username, ShareMode::Read)
        .await
        .unwrap();
}

#[tokio::test]
async fn share_unowned_file_write() {
    let cores = [
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

    let folder0 = cores[0]
        .create_file("folder0", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let result = cores[1]
        .share_file(folder0.id, &accounts[2].username, ShareMode::Write)
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn reject_share() {
    let cores = [
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

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[1].reject_share(&folder.id).await.unwrap();

    assert::all_pending_shares(&cores[1], &[]).await;
}

#[tokio::test]
async fn delete_link_to_share() {
    let cores = [
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

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    assert::all_pending_shares(&cores[1], &["folder"]).await;

    let link = cores[1]
        .create_link_at_path("link/", folder.id)
        .await
        .unwrap();

    assert::all_pending_shares(&cores[1], &[]).await;

    cores[1].delete(&link.id).await.unwrap();

    assert::all_pending_shares(&cores[1], &["folder"]).await;
}

#[tokio::test]
async fn create_link_with_deleted_duplicate() {
    let cores = [
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

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let link = cores[1]
        .create_link_at_path("link/", folder.id)
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();

    cores[1].delete(&link.id).await.unwrap();

    cores[1].sync(None).await.unwrap();

    let link2 = cores[1]
        .create_link_at_path("link/", folder.id)
        .await
        .unwrap();
    assert::all_pending_shares(&cores[1], &[]).await;

    cores[1].sync(None).await.unwrap();

    // note: originally, the new link is considered a duplicate during merge conflict resolution and is deleted; both the following assertions failed
    assert::all_pending_shares(&cores[1], &[]).await;
    cores[1].get_file_by_id(link2.id).await.unwrap();
}

#[tokio::test]
async fn reject_share_root() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();

    let result = core.reject_share(&root.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::RootModificationInvalid);
}

#[tokio::test]
async fn reject_share_duplicate() {
    let cores = [
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

    let folder0 = cores[0]
        .create_file("folder0", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[1].reject_share(&folder0.id).await.unwrap();

    let result = cores[1].reject_share(&folder0.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::ShareNonexistent);
}

#[tokio::test]
async fn reject_share_nonexistent() {
    let cores = [
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

    let folder0 = cores[0]
        .create_file("folder0", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[1].reject_share(&folder0.id).await.unwrap();

    let result = cores[1].reject_share(&folder0.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::ShareNonexistent);
}

#[tokio::test]
async fn create_at_path_insufficient_permission() {
    let core1 = test_core_with_account().await;
    let account1 = core1.get_account().unwrap();

    let core2 = test_core_with_account().await;
    let folder = core2.create_at_path("shared-folder/").await.unwrap();
    core2
        .share_file(folder.id, &account1.username, ShareMode::Read)
        .await
        .unwrap();
    core2.sync(None).await.unwrap();

    core1.sync(None).await.unwrap();
    core1
        .create_link_at_path("/received-folder", folder.id)
        .await
        .unwrap();

    let result = core1.create_at_path("received-folder/document").await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn get_path_by_id_link() {
    let core1 = test_core_with_account().await;
    let account1 = core1.get_account().unwrap();

    let core2 = test_core_with_account().await;
    let folder = core2.create_at_path("shared-folder/").await.unwrap();
    core2
        .share_file(folder.id, &account1.username, ShareMode::Read)
        .await
        .unwrap();
    core2.sync(None).await.unwrap();

    core1.sync(None).await.unwrap();
    let link = core1
        .create_link_at_path("received-folder", folder.id)
        .await
        .unwrap();

    assert_eq!(core1.get_path_by_id(link.id).await.unwrap(), "/received-folder/");
}

#[tokio::test]
async fn create_link_at_path_target_is_owned() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let document = core
        .create_file("document0", &root.id, FileType::Document)
        .await
        .unwrap();

    let result = core.create_link_at_path("link", document.id).await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::OwnedLink(_))
    );
}

#[tokio::test]
async fn create_link_at_path_target_nonexistent() {
    let core = test_core_with_account().await;

    let result = core.create_link_at_path("link", Uuid::new_v4()).await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::BrokenLink(_))
    );
}

#[tokio::test]
async fn create_link_at_path_link_in_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document0 = cores[0]
        .create_file("document0", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    let folder0 = cores[0]
        .create_file("folder0", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(document0.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0]
        .share_file(folder0.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder0.id })
        .await
        .unwrap();

    let result = cores[1]
        .create_link_at_path("folder_link/document", document0.id)
        .await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::SharedLink { .. })
    );
}

#[tokio::test]
async fn create_link_at_path_link_duplicate() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document0 = cores[0]
        .create_file("document0", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(document0.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_link_at_path("/link1", document0.id)
        .await
        .unwrap();

    let result = cores[1].create_link_at_path("/link2", document0.id).await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::DuplicateLink { .. })
    );
}

#[tokio::test]
async fn create_file_link_target_nonexistent() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();

    let result = core
        .create_file("link", &root.id, FileType::Link { target: Uuid::new_v4() })
        .await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::BrokenLink(_))
    );
}

#[tokio::test]
async fn create_file_link_target_owned() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();

    let document = core
        .create_file("document", &root.id, FileType::Document)
        .await
        .unwrap();

    let result = core
        .create_file("link", &root.id, FileType::Link { target: document.id })
        .await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::OwnedLink(_))
    );
}

#[tokio::test]
async fn create_file_shared_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document = cores[0]
        .create_file("document", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    let result = cores[1]
        .create_file("document_link", &folder.id, FileType::Link { target: document.id })
        .await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::SharedLink { .. })
    );
}

#[tokio::test]
async fn create_file_duplicate_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document = cores[0]
        .create_file("document", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link_1", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();

    let result = cores[1]
        .create_file("link_2", &roots[1].id, FileType::Link { target: document.id })
        .await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::DuplicateLink { .. })
    );
}

#[tokio::test]
async fn create_file_duplicate_link_deleted() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let document = cores[0]
        .create_file("document", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_file("link_1", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();

    cores[1].delete(&link.id).await.unwrap();

    cores[1]
        .create_file("link_2", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();
}

#[tokio::test]
async fn create_file_in_read_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    let result = cores[1]
        .create_file("document", &folder.id, FileType::Document)
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn create_file_in_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    cores[1]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();
}

#[tokio::test]
async fn rename_file_in_read_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    let result = cores[1].rename_file(&document.id, "renamed-document").await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn rename_file_in_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    cores[1]
        .rename_file(&document.id, "renamed-document")
        .await
        .unwrap();
}

#[tokio::test]
async fn rename_link_by_sharee() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    cores[1].rename_file(&link.id, "renamed").await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    assert::all_paths(&cores[1], &["/", "/renamed/"]).await;
    assert::all_paths(&cores[0], &["/", "/folder/"]).await;
}

#[tokio::test]
async fn rename_target_by_sharee() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    cores[1].rename_file(&folder.id, "renamed").await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    assert::all_paths(&cores[1], &["/", "/renamed/"]).await;
    assert::all_paths(&cores[0], &["/", "/folder/"]).await;
}

#[tokio::test]
async fn rename_target_by_sharer() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    cores[0].rename_file(&folder.id, "renamed").await.unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    assert::all_paths(&cores[1], &["/", "/link/"]).await;
    assert::all_paths(&cores[0], &["/", "/renamed/"]).await;
}

#[tokio::test]
async fn move_file_under_target() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();

    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    let document = cores[1]
        .create_file("document", &roots[1].id, FileType::Document)
        .await
        .unwrap();

    cores[1].move_file(&document.id, &folder.id).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]).await;
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document"]).await;
}

#[tokio::test]
async fn move_file_under_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();

    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_file("link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    let document = cores[1]
        .create_file("document", &roots[1].id, FileType::Document)
        .await
        .unwrap();

    let result = cores[1]
        .move_file(&document.id, &link.id)
        .await
        .unwrap_err();

    assert_matches!(
        result.kind,
        LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(_))
    );
}

#[tokio::test]
async fn move_file_shared_link() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    let document_link = cores[1]
        .create_file("document_link", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();

    let result = cores[1].move_file(&document_link.id, &folder.id).await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::SharedLink { .. })
    );
}

#[tokio::test]
async fn move_file_shared_link_in_folder_a() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    let child_folder = cores[1]
        .create_file("child_folder", &folder.id, FileType::Folder)
        .await
        .unwrap();
    let document_link = cores[1]
        .create_file("document_link", &roots[1].id, FileType::Link { target: document.id })
        .await
        .unwrap();

    let result = cores[1]
        .move_file(&document_link.id, &child_folder.id)
        .await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::SharedLink { .. })
    );
}

#[tokio::test]
async fn move_file_shared_link_in_folder_b() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &roots[0].id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    let child_folder = cores[1]
        .create_file("child_folder", &roots[1].id, FileType::Folder)
        .await
        .unwrap();
    cores[1]
        .create_file("document_link", &child_folder.id, FileType::Link { target: document.id })
        .await
        .unwrap();

    let result = cores[1].move_file(&child_folder.id, &folder.id).await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::SharedLink { .. })
    );
}

#[tokio::test]
async fn move_file_in_read_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let child_folder = cores[0]
        .create_file("folder", &folder.id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    let result = cores[1].move_file(&document.id, &child_folder.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn move_file_in_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let child_folder = cores[0]
        .create_file("folder", &folder.id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    cores[1]
        .move_file(&document.id, &child_folder.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn move_file_into_read_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    let document = cores[1]
        .create_file("document", &roots[1].id, FileType::Document)
        .await
        .unwrap();

    let result = cores[1].move_file(&document.id, &folder.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn move_file_into_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    let document = cores[1]
        .create_file("document", &roots[1].id, FileType::Document)
        .await
        .unwrap();
    cores[1].move_file(&document.id, &folder.id).await.unwrap();
}

#[tokio::test]
async fn move_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let child_folder = cores[1]
        .create_file("child_folder", &roots[1].id, FileType::Folder)
        .await
        .unwrap();

    let result = cores[1].move_file(&folder.id, &child_folder.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn delete_in_read_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    let result = cores[1].delete(&document.id).await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn delete_in_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    let document = cores[0]
        .create_file("document", &folder.id, FileType::Document)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();
    cores[1].delete(&document.id).await.unwrap();
}

// todo: check if duplicate
#[tokio::test]
async fn delete_write_shared_folder() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0]
        .create_file("folder", &roots[0].id, FileType::Folder)
        .await
        .unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let link = cores[1]
        .create_file("folder_link", &roots[1].id, FileType::Link { target: folder.id })
        .await
        .unwrap();

    cores[1].delete(&link.id).await.unwrap();
}

#[tokio::test]
async fn delete_share() {
    let core1 = test_core_with_account().await;
    let core2 = test_core_with_account().await;

    let core2_name = core2.get_account().unwrap().username.clone();

    let f1 = core1.create_at_path("a.md").await.unwrap();
    core1
        .share_file(f1.id, &core2_name, ShareMode::Write)
        .await
        .unwrap();
    core1.sync(None).await.unwrap();
    core2.sync(None).await.unwrap();

    core1.delete(&f1.id).await.unwrap();
    let f2 = core1.create_at_path("a.md").await.unwrap();
    core1
        .share_file(f2.id, &core2_name, ShareMode::Write)
        .await
        .unwrap();
    core1.sync(None).await.unwrap();
    core2.sync(None).await.unwrap();
}

#[tokio::test]
async fn delete_folder_with_shared_child() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    let document = cores[0].create_at_path("folder/document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();

    cores[0].delete(&folder.id).await.unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
}

#[tokio::test]
async fn populate_last_modified_by() {
    let c1 = test_core_with_account().await;
    let a1 = c1.get_account().unwrap();

    let c2 = test_core_with_account().await;
    let a2 = c2.get_account().unwrap();

    let doc = c1.create_at_path("/doc.md").await.unwrap();
    c1.share_file(doc.id, &a2.username, ShareMode::Write)
        .await
        .unwrap();
    c1.sync(None).await.unwrap();
    c2.sync(None).await.unwrap();

    assert_eq!(doc.last_modified_by, a1.username);
    let doc = c1.get_file_by_id(doc.id).await.unwrap();
    assert_eq!(doc.last_modified_by, a1.username);
    let doc = c2.get_file_by_id(doc.id).await.unwrap();
    assert_eq!(doc.last_modified_by, a1.username);

    c2.write_document(doc.id, b"a2's creative changes")
        .await
        .unwrap();
    let doc = c2.get_file_by_id(doc.id).await.unwrap();
    assert_eq!(doc.last_modified_by, a2.username);

    c2.sync(None).await.unwrap();
    c1.sync(None).await.unwrap();

    let doc = c1.get_file_by_id(doc.id).await.unwrap();
    assert_eq!(doc.last_modified_by, a2.username);
    let doc = c2.get_file_by_id(doc.id).await.unwrap();
    assert_eq!(doc.last_modified_by, a2.username);
}
