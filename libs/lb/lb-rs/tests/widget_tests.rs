use lb_rs::model::errors::LbErrKind;
use lb_rs::service::pin::read_pinned_documents;
use std::{fs, path::Path};
use test_utils::test_core_with_account;

#[tokio::test]
async fn widget_reads_current_pinned_names() {
    let core = test_core_with_account().await;
    let path = Path::new(&core.config.writeable_path);
    let doc = core.create_at_path("hello.md").await.unwrap();
    core.create_at_path("not-pinned.md").await.unwrap();
    core.pin_file(doc.id).await.unwrap();

    let files = read_pinned_documents(path).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!((files[0].id, files[0].name.as_str()), (doc.id, "hello.md"));

    core.rename_file(&doc.id, "renamed.md").await.unwrap();
    assert_eq!(read_pinned_documents(path).unwrap()[0].name, "renamed.md");

    core.delete(&doc.id).await.unwrap();
    assert!(read_pinned_documents(path).unwrap().is_empty());
}

#[test]
fn widget_does_not_create_or_migrate_an_account() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    fs::write(path.join("CoreV4.db"), b"leave legacy data to the app").unwrap();

    assert_eq!(read_pinned_documents(path).unwrap_err().kind, LbErrKind::AccountNonexistent);
    assert!(!path.join("CoreV5").exists());
    assert_eq!(fs::read(path.join("CoreV4.db")).unwrap(), b"leave legacy data to the app");

    fs::create_dir(path.join("CoreV5")).unwrap();
    assert_eq!(read_pinned_documents(path).unwrap_err().kind, LbErrKind::AccountNonexistent);
    assert_eq!(fs::read_dir(path.join("CoreV5")).unwrap().count(), 0);
}
