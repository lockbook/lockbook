use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::errors::Warning::*;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType::Document;
use lb_rs::model::secret_filename::SecretFileName;
use lb_rs::model::tree_like::TreeLike;
use rand::Rng;
use test_utils::*;

#[tokio::test]
async fn test_integrity_no_problems() {
    let core = test_core_with_account().await;
    core.test_repo_integrity().await.unwrap();
}

#[tokio::test]
async fn test_integrity_no_problems_but_more_complicated() {
    let core = test_core_with_account().await;
    core.create_at_path("test.md").await.unwrap();
    core.test_repo_integrity().await.unwrap();
}

#[tokio::test]
async fn test_no_account() {
    let core = test_core().await;
    assert_matches!(
        core.test_repo_integrity().await.unwrap_err().kind,
        LbErrKind::AccountNonexistent
    );
}

#[tokio::test]
async fn test_no_root() {
    let core = test_core_with_account().await;
    let mut tx = core.begin_tx().await;
    tx.db().base_metadata.clear().unwrap();
    tx.db().root.clear().unwrap();
    tx.end();
    assert_matches!(core.test_repo_integrity().await.unwrap_err().kind, LbErrKind::RootNonexistent);
}

#[tokio::test]
async fn test_orphaned_children() {
    let core = test_core_with_account().await;

    core.create_at_path("folder1/folder2/document1.md")
        .await
        .unwrap();
    core.test_repo_integrity().await.unwrap();

    let parent = core.get_by_path("folder1").await.unwrap().id;
    core.begin_tx()
        .await
        .db()
        .local_metadata
        .remove(&parent)
        .unwrap();
    assert_matches!(core.test_repo_integrity().await.unwrap_err().kind, LbErrKind::Validation(_));
}

#[tokio::test]
async fn test_invalid_file_name_slash() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("document1.md").await.unwrap();
    let mut tx = core.begin_tx().await;
    let db = tx.db();
    let mut tree = db.base_metadata.stage(&mut db.local_metadata).to_lazy();
    let key = tree.decrypt_key(&doc.id, &core.keychain).unwrap();
    let parent = tree.decrypt_key(&doc.parent, &core.keychain).unwrap();
    let new_name = SecretFileName::from_str("te/st", &key, &parent).unwrap();
    let mut doc = tree.find(&doc.id).unwrap().clone();
    doc.timestamped_value.value.set_name(new_name);
    tree.stage(Some(doc)).promote().unwrap();

    tx.end();

    assert_matches!(
        core.test_repo_integrity().await.unwrap_err().kind,
        LbErrKind::FileNameContainsSlash
    );
}

#[tokio::test]
async fn empty_filename() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("document1.md").await.unwrap();
    let mut tx = core.begin_tx().await;
    let db = tx.db();
    let mut tree = db.base_metadata.stage(&mut db.local_metadata).to_lazy();
    let key = tree.decrypt_key(&doc.id, &core.keychain).unwrap();
    let parent = tree.decrypt_key(&doc.parent, &core.keychain).unwrap();
    let new_name = SecretFileName::from_str("", &key, &parent).unwrap();
    let mut doc = tree.find(&doc.id).unwrap().clone();
    doc.timestamped_value.value.set_name(new_name);
    tree.stage(Some(doc)).promote().unwrap();

    tx.end();

    assert_matches!(core.test_repo_integrity().await.unwrap_err().kind, LbErrKind::FileNameEmpty);
}

#[tokio::test]
async fn test_cycle() {
    let core = test_core_with_account().await;
    core.create_at_path("folder1/folder2/document1.md")
        .await
        .unwrap();
    let parent = core.get_by_path("folder1").await.unwrap().id;
    core.begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&parent)
        .unwrap();
    let mut parent = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&parent)
        .unwrap()
        .clone();
    let child = core.get_by_path("folder1/folder2").await.unwrap();
    parent.timestamped_value.value.set_parent(child.id);
    core.begin_tx()
        .await
        .db()
        .local_metadata
        .insert(*parent.id(), parent)
        .unwrap();
    assert_matches!(
        core.test_repo_integrity().await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::Cycle(_))
    );
}

#[tokio::test]
async fn test_documents_treated_as_folders() {
    let core = test_core_with_account().await;
    core.create_at_path("folder1/folder2/document1.md")
        .await
        .unwrap();
    let parent = core.get_by_path("folder1").await.unwrap();
    let mut parent = core
        .begin_tx()
        .await
        .db()
        .local_metadata
        .get()
        .get(&parent.id)
        .unwrap()
        .clone();
    parent.timestamped_value.value.set_type(Document);
    core.begin_tx()
        .await
        .db()
        .local_metadata
        .insert(*parent.id(), parent)
        .unwrap();
    assert_matches!(
        core.test_repo_integrity().await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(_))
    );
}

#[tokio::test]
async fn test_name_conflict() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("document1.md").await.unwrap();
    core.create_at_path("document2.md").await.unwrap();
    let mut tx = core.begin_tx().await;
    let db = tx.db();
    let mut tree = db.base_metadata.stage(&mut db.local_metadata).to_lazy();
    let key = tree.decrypt_key(&doc.id, &core.keychain).unwrap();
    let parent = tree.decrypt_key(&doc.parent, &core.keychain).unwrap();
    let new_name = SecretFileName::from_str("document2.md", &key, &parent).unwrap();
    let mut doc = tree.find(&doc.id).unwrap().clone();
    doc.timestamped_value.value.set_name(new_name);
    tree.stage(Some(doc)).promote().unwrap();

    tx.end();

    assert_matches!(
        core.test_repo_integrity().await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::PathConflict(_))
    );
}

#[tokio::test]
async fn test_empty_file() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("document.txt").await.unwrap();
    core.write_document(doc.id, &[]).await.unwrap();
    let warnings = core.test_repo_integrity().await;

    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([EmptyFile(_)]));
}

#[tokio::test]
async fn test_invalid_utf8() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("document.txt").await.unwrap();
    core.write_document(doc.id, rand::thread_rng().r#gen::<[u8; 32]>().as_ref())
        .await
        .unwrap();
    let warnings = core.test_repo_integrity().await;
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([InvalidUTF8(_)]));
}

#[tokio::test]
async fn test_invalid_utf8_ignores_non_utf_file_extensions() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("document.png").await.unwrap();
    core.write_document(doc.id, rand::thread_rng().r#gen::<[u8; 32]>().as_ref())
        .await
        .unwrap();
    let warnings = core.test_repo_integrity().await;
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([]));
}
