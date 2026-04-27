use lb_bifs::{BASE_DIR, BiFS, DATA_DIR, SYNC_FOLDER, compute_hash};
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
async fn push_no_changes() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // push with no changes
    bifs.push().await;

    // file should be unchanged in lockbook
    let content = lb.read_document(doc.id, false).await.unwrap();
    assert_eq!(content, b"content");
}

#[tokio::test]
async fn push_local_changes() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"original").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // modify local file
    fs::write(root.join("test.txt"), b"modified").unwrap();

    // push changes
    bifs.push().await;

    // lockbook should have the new content
    lb.sync().await.unwrap();
    let content = lb.read_document(doc.id, false).await.unwrap();
    assert_eq!(content, b"modified");

    // index should be updated with new hash
    assert_eq!(bifs.index.files.get(&doc.id).unwrap().hash, compute_hash(b"modified"));
}

#[tokio::test]
async fn push_locally_deleted_file() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;
    assert!(bifs.index.files.contains_key(&doc.id));

    // delete local file
    fs::remove_file(root.join("test.txt")).unwrap();

    // push - should delete from lockbook
    bifs.push().await;

    // file should be deleted in lockbook
    lb.sync().await.unwrap();
    assert!(
        lb.get_by_path(&format!("{}/test.txt", SYNC_FOLDER))
            .await
            .is_err()
    );

    // index entry should be removed
    assert!(!bifs.index.files.contains_key(&doc.id));
}

#[tokio::test]
async fn push_locally_relocated_file_deletes_from_lockbook() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"content").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // relocate file on disk
    fs::create_dir_all(root.join("moved")).unwrap();
    fs::rename(root.join("test.txt"), root.join("moved/test.txt")).unwrap();

    // push - relocated file should be treated as deleted
    bifs.push().await;

    // file should be deleted in lockbook
    lb.sync().await.unwrap();
    assert!(
        lb.get_by_path(&format!("{}/test.txt", SYNC_FOLDER))
            .await
            .is_err()
    );

    // index entry should be removed
    assert!(!bifs.index.files.contains_key(&doc.id));
}

#[tokio::test]
async fn push_multiple_changes() {
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
        .create_at_path(&format!("{}/file3.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc3.id, b"content 3").await.unwrap();

    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    // modify file1
    fs::write(root.join("file1.txt"), b"modified 1").unwrap();
    // delete file2
    fs::remove_file(root.join("file2.txt")).unwrap();
    // leave file3 unchanged

    bifs.push().await;
    lb.sync().await.unwrap();

    // file1 should be modified
    assert_eq!(lb.read_document(doc1.id, false).await.unwrap(), b"modified 1");
    // file2 should be deleted
    assert!(
        lb.get_by_path(&format!("{}/file2.txt", SYNC_FOLDER))
            .await
            .is_err()
    );
    // file3 should be unchanged
    assert_eq!(lb.read_document(doc3.id, false).await.unwrap(), b"content 3");
}

#[tokio::test]
async fn push_creates_untracked_file_in_lockbook() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await; // ensure sync folder exists

    // create an untracked file on disk
    fs::write(root.join("new_file.txt"), b"new content").unwrap();

    // push
    bifs.push().await;

    // file should now be in lockbook
    let doc = lb
        .get_by_path(&format!("{}/new_file.txt", SYNC_FOLDER))
        .await
        .unwrap();
    let content = lb.read_document(doc.id, false).await.unwrap();
    assert_eq!(content, b"new content");

    // file should be in index
    assert_eq!(bifs.index.files.len(), 1);
    assert!(bifs.index.files.contains_key(&doc.id));
    assert_eq!(bifs.index.files.get(&doc.id).unwrap().path, "new_file.txt");
}

#[tokio::test]
async fn push_merges_concurrent_edits() {
    let lb = test_core_with_account().await;
    let root = test_root();

    // create file in lockbook with base content
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

    // local edit: change line 2
    fs::write(root.join("test.txt"), b"line 1\nlocal edit\nline 3").unwrap();

    // remote edit: change line 3
    lb.write_document(doc.id, b"line 1\nline 2\nremote edit")
        .await
        .unwrap();
    lb.sync().await.unwrap();

    // push should merge both edits
    bifs.push().await;

    // lockbook should have merged content
    lb.sync().await.unwrap();
    let content = lb.read_document(doc.id, false).await.unwrap();
    let content_str = String::from_utf8_lossy(&content);
    assert!(content_str.contains("local edit"), "should contain local edit");
    assert!(content_str.contains("remote edit"), "should contain remote edit");
}

#[tokio::test]
async fn push_updates_index_and_base() {
    let lb = test_core_with_account().await;
    let root = test_root();

    let doc = lb
        .create_at_path(&format!("{}/test.txt", SYNC_FOLDER))
        .await
        .unwrap();
    lb.write_document(doc.id, b"original").await.unwrap();
    lb.sync().await.unwrap();

    let mut bifs = BiFS::new(lb.clone(), root.clone());
    bifs.pull().await;

    let old_hash = bifs.index.files.get(&doc.id).unwrap().hash.clone();

    // modify and push
    fs::write(root.join("test.txt"), b"modified").unwrap();
    bifs.push().await;

    let new_hash = bifs.index.files.get(&doc.id).unwrap().hash.clone();

    // hash should be updated
    assert_ne!(old_hash, new_hash);
    // old base should be deleted
    assert!(!root.join(DATA_DIR).join(BASE_DIR).join(&old_hash).exists());
    // new base should exist
    assert!(root.join(DATA_DIR).join(BASE_DIR).join(&new_hash).exists());
}
