use lb_bifs::{BASE_DIR, BiFS, DATA_DIR, INDEX_FILE, SYNC_FOLDER, compute_hash};
use lb_rs::Uuid;
use std::fs;
use std::path::PathBuf;
use test_utils::*;

fn test_root() -> PathBuf {
    let path = PathBuf::from(format!("/tmp/lb-bifs-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

#[tokio::test]
async fn init_creates_data_dir() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let _bifs = BiFS::new(lb, root.clone());

    assert!(root.join(DATA_DIR).exists());
    assert!(root.join(DATA_DIR).join(BASE_DIR).exists());
}

#[tokio::test]
async fn init_loads_empty_index() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let bifs = BiFS::new(lb, root);

    assert!(bifs.index.files.is_empty());
}

#[tokio::test]
async fn pull_creates_sync_folder_in_lockbook() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let mut bifs = BiFS::new(lb, root);
    bifs.pull().await;

    let folder = bifs.lb.get_by_path(SYNC_FOLDER).await.unwrap();
    assert!(folder.is_folder());
}

#[tokio::test]
async fn pull_empty_folder() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let mut bifs = BiFS::new(lb, root.clone());
    bifs.pull().await;

    assert!(bifs.index.files.is_empty());
    // index file should still be created
    assert!(root.join(DATA_DIR).join(INDEX_FILE).exists());
}

#[tokio::test]
async fn pull_single_document() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // create a document in the sync folder
    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    let content = b"hello world";
    lb.write_document(doc.id, content).await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb, root.clone());
    bifs.pull().await;

    // check file on disk
    let local_content = fs::read(root.join("test.txt")).unwrap();
    assert_eq!(local_content, content);

    // check index
    assert_eq!(bifs.index.files.len(), 1);
    let record = bifs.index.files.get(&doc.id).unwrap();
    assert_eq!(record.path, "test.txt");
    assert_eq!(record.hash, compute_hash(content));

    // check base file exists
    assert!(
        root.join(DATA_DIR)
            .join(BASE_DIR)
            .join(&record.hash)
            .exists()
    );
}

#[tokio::test]
async fn pull_document_in_subfolder() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/sub/folder/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    let content = b"nested content";
    lb.write_document(doc.id, content).await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb, root.clone());
    bifs.pull().await;

    let local_content = fs::read(root.join("sub/folder/test.txt")).unwrap();
    assert_eq!(local_content, content);

    let record = bifs.index.files.get(&doc.id).unwrap();
    assert_eq!(record.path, "sub/folder/test.txt");
}

#[tokio::test]
async fn pull_multiple_documents() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc1 = lb
        .create_at_path(&format!("{}/file1.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc1.id, b"content 1").await.unwrap();

    let doc2 = lb
        .create_at_path(&format!("{}/file2.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc2.id, b"content 2").await.unwrap();

    let doc3 = lb
        .create_at_path(&format!("{}/sub/file3.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc3.id, b"content 3").await.unwrap();

    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb, root.clone());
    bifs.pull().await;

    assert_eq!(bifs.index.files.len(), 3);
    assert_eq!(fs::read(root.join("file1.txt")).unwrap(), b"content 1");
    assert_eq!(fs::read(root.join("file2.txt")).unwrap(), b"content 2");
    assert_eq!(fs::read(root.join("sub/file3.txt")).unwrap(), b"content 3");
}

#[tokio::test]
async fn pull_updates_unchanged_local_file() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // initial pull
    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"version 1").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;
    assert_eq!(fs::read(root.join("test.txt")).unwrap(), b"version 1");

    // update on remote
    lb.write_document(doc.id, b"version 2").await.unwrap();
    lb.sync().await.unwrap();

    // second pull - local file unchanged, should overwrite
    let mut bifs = BiFS::new(lb, root.clone());
    bifs.pull().await;
    assert_eq!(fs::read(root.join("test.txt")).unwrap(), b"version 2");
}

#[tokio::test]
async fn pull_merges_changed_local_file() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // initial pull
    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"line 1\nline 2\nline 3")
        .await
        .unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // modify local file
    fs::write(root.join("test.txt"), b"line 1\nlocal change\nline 3").unwrap();

    // modify remote
    lb.write_document(doc.id, b"line 1\nline 2\nremote change")
        .await
        .unwrap();
    lb.sync().await.unwrap();

    // second pull - should merge
    let mut bifs = BiFS::new(lb, root.clone());
    bifs.pull().await;

    let merged = fs::read_to_string(root.join("test.txt")).unwrap();
    // the merge should contain both changes
    assert!(merged.contains("local change"));
    assert!(merged.contains("remote change"));
}

#[tokio::test]
async fn index_persists_across_instances() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    // first instance
    {
        let mut bifs = BiFS::new(lb.clone(), root.clone());
        bifs.pull().await;
        assert_eq!(bifs.index.files.len(), 1);
    }

    // second instance should load saved index
    {
        let bifs = BiFS::new(lb, root.clone());
        assert_eq!(bifs.index.files.len(), 1);
        assert!(bifs.index.files.contains_key(&doc.id));
    }
}

#[tokio::test]
async fn old_base_deleted_after_pull() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"version 1").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    let old_hash = bifs.index.files.get(&doc.id).unwrap().hash.clone();
    assert!(root.join(DATA_DIR).join(BASE_DIR).join(&old_hash).exists());

    // update remote
    lb.write_document(doc.id, b"version 2").await.unwrap();
    lb.sync().await.unwrap();

    // pull again
    let mut bifs = BiFS::new(lb, root.clone());
    bifs.pull().await;

    let new_hash = bifs.index.files.get(&doc.id).unwrap().hash.clone();

    // old base should be deleted, new base should exist
    assert!(!root.join(DATA_DIR).join(BASE_DIR).join(&old_hash).exists());
    assert!(root.join(DATA_DIR).join(BASE_DIR).join(&new_hash).exists());
}

#[tokio::test]
async fn pull_stores_hmac() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb, root);
    bifs.pull().await;

    let record = bifs.index.files.get(&doc.id).unwrap();
    assert!(record.hmac.is_some());
}

#[tokio::test]
async fn pull_skips_relocated_file() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // initial pull
    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;
    assert!(root.join("test.txt").exists());

    // relocate file on disk
    fs::create_dir_all(root.join("moved")).unwrap();
    fs::rename(root.join("test.txt"), root.join("moved/test.txt")).unwrap();
    assert!(!root.join("test.txt").exists());

    // update remote
    lb.write_document(doc.id, b"updated content").await.unwrap();
    lb.sync().await.unwrap();

    // pull again - should skip the relocated file, not recreate it
    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // file should not be recreated at original path
    assert!(!root.join("test.txt").exists());
    // relocated file should be untouched
    assert_eq!(fs::read(root.join("moved/test.txt")).unwrap(), b"content");
    // file should still exist in lockbook
    assert!(
        lb.get_by_path(&format!("{}/test.txt", SYNC_FOLDER))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn pull_handles_file_relocated_in_lockbook() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // initial pull
    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;
    assert!(root.join("test.txt").exists());
    assert_eq!(bifs.index.files.get(&doc.id).unwrap().path, "test.txt");

    // relocate file in lockbook
    let moved_folder = lb
        .create_at_path(&format!("{}/moved/", SYNC_FOLDER))
        .await
        .unwrap();
    lb.move_file(&doc.id, &moved_folder.id).await.unwrap();
    lb.sync().await.unwrap();

    // pull again - should move local file to new location
    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // old location should be gone
    assert!(!root.join("test.txt").exists());
    // file should be at new location
    assert!(root.join("moved/test.txt").exists());
    assert_eq!(fs::read(root.join("moved/test.txt")).unwrap(), b"content");
    // index should reflect new path
    assert_eq!(bifs.index.files.get(&doc.id).unwrap().path, "moved/test.txt");
}

#[tokio::test]
async fn pull_handles_file_deleted_in_lockbook() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // initial pull
    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;
    assert!(root.join("test.txt").exists());
    assert!(bifs.index.files.contains_key(&doc.id));

    // delete file in lockbook
    lb.delete(&doc.id).await.unwrap();
    lb.sync().await.unwrap();

    // pull again - should delete local file
    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // file should be deleted locally
    assert!(!root.join("test.txt").exists());
    // index entry should be removed
    assert!(!bifs.index.files.contains_key(&doc.id));
}

#[tokio::test]
async fn pull_deletion_is_final_even_with_local_changes() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // initial pull
    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"original content")
        .await
        .unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // modify local file
    fs::write(root.join("test.txt"), b"local changes that will be lost").unwrap();

    // delete file in lockbook
    lb.delete(&doc.id).await.unwrap();
    lb.sync().await.unwrap();

    // pull again - deletion should win over local changes
    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // file should be deleted despite local changes
    assert!(!root.join("test.txt").exists());
    assert!(!bifs.index.files.contains_key(&doc.id));
}
