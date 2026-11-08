use lb_rs::model::file_metadata::FileType;
use lb_rs::service::import_export::{Action, EntryType, Existing, ExportFileInfo, ImportStatus};
use rand::Rng;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use test_utils::{test_core_from, test_core_with_account, url};
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

#[tokio::test]
async fn test_get_file_link_url() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("test.md").await.unwrap().id;

    let link_url = core.get_file_link_url(id).await.unwrap();
    let expected_link = format!("{}/open/{}", url(), id);

    assert_eq!(expected_link, link_url);
}

/// A folder of notes with what a file browser hides beside them.
fn trip() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let trip = tmp.path().join("Trip");
    std::fs::create_dir_all(trip.join("Photos")).unwrap();
    std::fs::create_dir_all(trip.join(".git")).unwrap();
    std::fs::write(trip.join("Packing.md"), "- socks").unwrap();
    std::fs::write(trip.join("Plan.md"), "fly out friday").unwrap();
    std::fs::write(trip.join("Photos/beach.jpg"), [7u8; 64]).unwrap();
    std::fs::write(trip.join(".DS_Store"), [0u8; 16]).unwrap();
    std::fs::write(trip.join(".git/HEAD"), "ref: refs/heads/master").unwrap();
    tmp
}

async fn read(core: &test_utils::Lb, path: &str) -> Vec<u8> {
    let id = core.get_by_path(path).await.unwrap().id;
    core.read_document(id, false).await.unwrap()
}

/// Importing what was just imported writes nothing, and a changed file is the one write after.
#[tokio::test]
async fn reimport_writes_only_what_changed() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let tmp = trip();
    let trip = tmp.path().join("Trip");

    core.import_files(std::slice::from_ref(&trip), root.id, &|_| {})
        .await
        .unwrap();
    assert_eq!(read(&core, "/Trip/Photos/beach.jpg").await, [7u8; 64]);
    assert!(core.get_by_path("/Trip/.DS_Store").await.is_err());
    assert!(core.get_by_path("/Trip/.git").await.is_err());

    let again = core
        .plan_import(std::slice::from_ref(&trip), root.id, Existing::Update)
        .await
        .unwrap();
    let writes: Vec<_> = again.entries.iter().filter(|e| e.writes()).collect();
    assert!(writes.is_empty(), "{writes:?}");

    std::fs::write(trip.join("Packing.md"), "- socks\n- hat").unwrap();
    let plan = core
        .plan_import(std::slice::from_ref(&trip), root.id, Existing::Update)
        .await
        .unwrap();
    let writes: Vec<_> = plan.entries.iter().filter(|e| e.writes()).collect();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert!(matches!(writes[0].action, Action::Replace(_)));
    assert_eq!(writes[0].name(), "Packing.md");

    core.import_planned(&plan, &|_| {}).await.unwrap();
    assert_eq!(read(&core, "/Trip/Packing.md").await, b"- socks\n- hat");
    assert_eq!(core.get_children(&root.id).await.unwrap().len(), 1);
}

/// Without a plan someone saw, an import never replaces anything: it arrives beside what's there.
#[tokio::test]
async fn import_files_copies_beside_what_exists() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let tmp = trip();
    let trip = tmp.path().join("Trip");

    core.import_files(std::slice::from_ref(&trip), root.id, &|_| {})
        .await
        .unwrap();
    std::fs::write(trip.join("Packing.md"), "- socks\n- hat").unwrap();
    core.import_files(std::slice::from_ref(&trip), root.id, &|_| {})
        .await
        .unwrap();

    assert_eq!(read(&core, "/Trip/Packing.md").await, b"- socks");
    assert_eq!(read(&core, "/Trip-1/Packing.md").await, b"- socks\n- hat");
    assert_eq!(core.get_children(&root.id).await.unwrap().len(), 2);
}

/// Without a plan to show it, a source that can't be read fails the import by name.
#[tokio::test]
async fn import_files_names_a_missing_source() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let tmp = trip();
    let missing = tmp.path().join("Gone.md");

    let err = core
        .import_files(&[tmp.path().join("Trip"), missing], root.id, &|_| {})
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Gone.md"), "{err}");
    assert_eq!(read(&core, "/Trip/Plan.md").await, b"fly out friday");
}

/// Re-importing notes whose content is only on the server downloads nothing and writes nothing.
#[tokio::test]
async fn reimport_compares_without_downloading() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let tmp = trip();
    let trip = tmp.path().join("Trip");
    core.import_files(std::slice::from_ref(&trip), root.id, &|_| {})
        .await
        .unwrap();
    core.sync().await.unwrap();

    let other = test_core_from(&core).await;
    let mut remote = Vec::new();
    for path in ["/Trip/Packing.md", "/Trip/Plan.md", "/Trip/Photos/beach.jpg"] {
        let id = other.get_by_path(path).await.unwrap().id;
        let (hmac, _) = other.read_document_with_hmac(id, false).await.unwrap();
        other.docs.delete(id, hmac).await.unwrap();
        remote.push((id, hmac));
    }

    let plan = other
        .plan_import(std::slice::from_ref(&trip), root.id, Existing::Update)
        .await
        .unwrap();
    let writes: Vec<_> = plan.entries.iter().filter(|e| e.writes()).collect();
    assert!(writes.is_empty(), "{writes:?}");
    for (id, hmac) in remote {
        assert!(other.docs.maybe_get(id, hmac).await.unwrap().is_none());
    }
}

/// Skipped documents stay on disk, and their folder still arrives.
#[tokio::test]
async fn skipped_entries_stay_out() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let tmp = trip();

    let mut plan = core
        .plan_import(&[tmp.path().join("Trip")], root.id, Existing::Update)
        .await
        .unwrap();
    for e in &mut plan.entries {
        e.skip =
            e.file_type != EntryType::Folder && e.disk_path.extension() == Some("jpg".as_ref());
    }
    assert_eq!(plan.summary().skipped.files, 1);
    core.import_planned(&plan, &|_| {}).await.unwrap();

    assert!(core.get_by_path("/Trip/Photos/beach.jpg").await.is_err());
    let photos = core.get_by_path("/Trip/Photos").await.unwrap();
    assert!(core.get_children(&photos.id).await.unwrap().is_empty());
    assert_eq!(read(&core, "/Trip/Plan.md").await, b"fly out friday");
}

/// Links, pipes and sockets are left behind without blocking the walk, and a picked link is
/// refused by name rather than followed.
#[cfg(unix)]
#[tokio::test]
async fn import_ignores_what_isnt_a_file() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let tmp = trip();
    let trip = tmp.path().join("Trip");
    std::os::unix::fs::symlink(trip.join("Plan.md"), trip.join("link.md")).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(trip.join("socket.md")).unwrap();
    let fifo = std::ffi::CString::new(trip.join("pipe.md").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);

    let plan = core
        .plan_import(std::slice::from_ref(&trip), root.id, Existing::Update)
        .await
        .unwrap();
    assert_eq!(plan.summary().not_a_file, 3);
    core.import_planned(&plan, &|_| {}).await.unwrap();
    for name in ["link.md", "socket.md", "pipe.md"] {
        assert!(core.get_by_path(&format!("/Trip/{name}")).await.is_err(), "{name}");
    }

    let picked = tmp.path().join("Trip link");
    std::os::unix::fs::symlink(&trip, &picked).unwrap();
    let plan = core
        .plan_import(std::slice::from_ref(&picked), root.id, Existing::Update)
        .await
        .unwrap();
    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.summary().not_a_file, 1);
    let err = core
        .import_files(&[picked], root.id, &|_| {})
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Trip link"), "{err}");
    assert!(core.get_by_path("/Trip link").await.is_err());
}

/// A file that can't be read when its turn comes is named in the error and leaves no note.
#[cfg(unix)]
#[tokio::test]
async fn failed_read_leaves_no_note() {
    use std::os::unix::fs::PermissionsExt;

    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let tmp = trip();
    let trip = tmp.path().join("Trip");

    let plan = core
        .plan_import(std::slice::from_ref(&trip), root.id, Existing::Update)
        .await
        .unwrap();
    std::fs::set_permissions(trip.join("Plan.md"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let err = core.import_planned(&plan, &|_| {}).await.unwrap_err();

    assert!(err.to_string().contains("Plan.md couldn't be imported"), "{err}");
    assert!(core.get_by_path("/Trip/Plan.md").await.is_err());
    assert_eq!(read(&core, "/Trip/Packing.md").await, b"- socks");
}
