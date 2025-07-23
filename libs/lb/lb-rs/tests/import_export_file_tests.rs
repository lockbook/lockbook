use lb_rs::model::file_metadata::FileType;
use lb_rs::service::import_export::{ExportFileInfo, ImportStatus};
use rand::Rng;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use test_utils::test_core_with_account;
use uuid::Uuid;

#[tokio::test]
async fn import_file_successfully() {
    let core = test_core_with_account().await;
    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().to_path_buf();

    // generating document in /tmp/
    let name = Uuid::new_v4().to_string();
    let doc_path = tmp_path.join(&name);

    std::fs::write(&doc_path, rand::thread_rng().r#gen::<[u8; 32]>()).unwrap();

    let root = core.root().await.unwrap();

    let f = move |status: ImportStatus| {
        // only checking if the disk path exists since a lockbook folder that has children won't
        // be created until its first child is
        match status {
            ImportStatus::CalculatedTotal(_) => {}
            ImportStatus::StartingItem(path_str) => {
                let disk_path = PathBuf::from(path_str);
                assert!(disk_path.exists());
            }
            ImportStatus::FinishedItem(_metadata) => {}
        }
    };

    core.import_files(&[doc_path], root.id, &f).await.unwrap();

    core.get_by_path(&format!("/{name}")).await.unwrap();

    // generating folder with a document in /tmp/
    let parent_name = Uuid::new_v4().to_string();
    let parent_path = tmp_path.join(&parent_name);

    std::fs::create_dir(&parent_path).unwrap();

    let child_name = Uuid::new_v4().to_string();
    let child_path = parent_path.join(&child_name);

    std::fs::write(child_path, rand::thread_rng().r#gen::<[u8; 32]>()).unwrap();

    core.import_files(&[parent_path], root.id, &f)
        .await
        .unwrap();

    core.get_by_path(&format!("/{parent_name}/{child_name}"))
        .await
        .unwrap();
}

#[tokio::test]
async fn export_file_successfully() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().to_path_buf();

    // generating document in lockbook
    let name = Uuid::new_v4().to_string();
    let file = core
        .create_file(&name, &root.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(file.id, &rand::thread_rng().r#gen::<[u8; 32]>())
        .await
        .unwrap();

    let paths: Arc<Mutex<Vec<ExportFileInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let path_copy = paths.clone();

    let export_progress = move |info: ExportFileInfo| {
        path_copy.lock().unwrap().push(info);
    };
    core.export_file(file.id, tmp_path.clone(), false, &Some(export_progress.clone()))
        .await
        .unwrap();
    // todo(parth): fix clippy warning await_holding_lock
    let paths = paths.lock().unwrap().clone();
    for info in paths.iter() {
        core.get_by_path(&info.lockbook_path).await.unwrap();
        assert!(info.disk_path.exists());
    }

    assert!(tmp_path.join(file.name).exists());

    // generating folder with a document in lockbook
    let parent_name = Uuid::new_v4().to_string();
    let child_name = Uuid::new_v4().to_string();
    let child = core
        .create_at_path(&format!("/{}/{}/{}", root.name, parent_name, child_name))
        .await
        .unwrap();

    core.write_document(child.id, &rand::thread_rng().r#gen::<[u8; 32]>())
        .await
        .unwrap();

    core.export_file(child.parent, tmp_path.clone(), false, &Some(export_progress))
        .await
        .unwrap();

    assert!(tmp_path.join(parent_name).join(child_name).exists());
}
