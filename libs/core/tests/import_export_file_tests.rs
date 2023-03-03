use lockbook_core::service::import_export_service::{ImportExportFileInfo, ImportStatus};
use lockbook_shared::file_metadata::FileType;
use rand::Rng;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use test_utils::test_core_with_account;
use uuid::Uuid;

#[test]
fn import_file_successfully() {
    let core = test_core_with_account();
    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().to_path_buf();

    // generating document in /tmp/
    let name = Uuid::new_v4().to_string();
    let doc_path = tmp_path.join(&name);

    std::fs::write(&doc_path, rand::thread_rng().gen::<[u8; 32]>()).unwrap();

    let root = core.get_root().unwrap();

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

    core.import_files(&[doc_path], root.id, &f).unwrap();

    core.get_by_path(&format!("/{}", name)).unwrap();

    // generating folder with a document in /tmp/
    let parent_name = Uuid::new_v4().to_string();
    let parent_path = tmp_path.join(&parent_name);

    std::fs::create_dir(&parent_path).unwrap();

    let child_name = Uuid::new_v4().to_string();
    let child_path = parent_path.join(&child_name);

    std::fs::write(child_path, rand::thread_rng().gen::<[u8; 32]>()).unwrap();

    core.import_files(&[parent_path], root.id, &f).unwrap();

    core.get_by_path(&format!("/{}/{}", parent_name, child_name))
        .unwrap();
}

#[test]
fn export_file_successfully() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().to_path_buf();

    // generating document in lockbook
    let name = Uuid::new_v4().to_string();
    let file = core
        .create_file(&name, root.id, FileType::Document)
        .unwrap();
    core.write_document(file.id, &rand::thread_rng().gen::<[u8; 32]>())
        .unwrap();

    let paths: Arc<Mutex<Vec<ImportExportFileInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let path_copy = paths.clone();

    let export_progress = move |info: ImportExportFileInfo| {
        path_copy.lock().unwrap().push(info);
    };
    core.export_file(file.id, tmp_path.clone(), false, Some(Box::new(export_progress.clone())))
        .unwrap();
    for info in paths.lock().unwrap().iter() {
        core.get_by_path(&info.lockbook_path).unwrap();
        assert!(info.disk_path.exists());
    }

    assert!(tmp_path.join(file.name).exists());

    // generating folder with a document in lockbook
    let parent_name = Uuid::new_v4().to_string();
    let child_name = Uuid::new_v4().to_string();
    let child = core
        .create_at_path(&format!("/{}/{}/{}", root.name, parent_name, child_name))
        .unwrap();

    core.write_document(child.id, &rand::thread_rng().gen::<[u8; 32]>())
        .unwrap();

    core.export_file(child.parent, tmp_path.clone(), false, Some(Box::new(export_progress)))
        .unwrap();

    assert!(tmp_path.join(parent_name).join(child_name).exists());
}
