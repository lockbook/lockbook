#![cfg(all(unix, not(target_os = "ios")))]

use lb_rs::Lb;
use lb_rs::model::account::Account;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{FileType, Owner};
use lb_rs::model::meta::Meta;
use lb_rs::service::import_export::ExportFileInfo;
use std::process::Command;
use test_utils::test_config;

#[test]
fn export_large_folders_with_low_file_limit() {
    // rlimits affect the whole process, so isolate this from other tests.
    const CHILD: &str = "LB_EXPORT_LOW_FD_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "export_large_folders_with_low_file_limit", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "low-file-limit export failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    unsafe {
        assert_eq!(libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit), 0);
        limit.rlim_cur = limit.rlim_cur.min(64);
        assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &limit), 0);
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let storage = tempfile::tempdir().unwrap();
            let mut config = test_config();
            config.writeable_path = storage.path().to_str().unwrap().to_owned();
            let core = Lb::init(config).await.unwrap();

            // A local account keeps this descriptor regression independent of a server.
            let account = Account::new("export-test".into(), "http://localhost:1".into());
            let root = Meta::create_root(&account)
                .unwrap()
                .sign_with(&account)
                .unwrap();
            let root_id = *root.id();
            core.keychain.cache_account(account.clone()).await.unwrap();
            let mut tx = core.begin_tx().await;
            let db = tx.db();
            db.account.replace(account.clone()).unwrap();
            db.root.replace(root_id).unwrap();
            db.base_metadata.insert(root_id, root).unwrap();
            db.pub_key_lookup
                .insert(Owner(account.public_key()), account.username.clone())
                .unwrap();
            tx.end();

            let folder = core
                .create_file("export", &root_id, FileType::Folder)
                .await
                .unwrap();
            let mut expected = Vec::new();
            // Exercise both a wide folder and concurrent exports in separate subfolders.
            for group in 0..9 {
                let (parent, prefix, count) = if group == 0 {
                    (folder.id, String::new(), 128)
                } else {
                    let name = format!("nested-{group}");
                    let nested = core
                        .create_file(&name, &folder.id, FileType::Folder)
                        .await
                        .unwrap();
                    (nested.id, format!("{name}/"), 16)
                };
                for index in 0..count {
                    let name = format!("{index}.txt");
                    let file = core
                        .create_file(&name, &parent, FileType::Document)
                        .await
                        .unwrap();
                    let content = format!("contents of {prefix}{name}");
                    core.write_document(file.id, content.as_bytes())
                        .await
                        .unwrap();
                    expected.push((format!("export/{prefix}{name}"), content));
                }
            }
            core.create_file("empty", &folder.id, FileType::Folder)
                .await
                .unwrap();

            let first = tempfile::tempdir().unwrap();
            let second = tempfile::tempdir().unwrap();
            let (a, b) = tokio::join!(
                core.export_file(
                    folder.id,
                    first.path().to_owned(),
                    false,
                    &None::<fn(ExportFileInfo)>
                ),
                core.export_file(
                    folder.id,
                    second.path().to_owned(),
                    false,
                    &None::<fn(ExportFileInfo)>
                ),
            );
            a.unwrap();
            b.unwrap();
            for dest in [first.path(), second.path()] {
                for (path, content) in &expected {
                    assert_eq!(std::fs::read(dest.join(path)).unwrap(), content.as_bytes());
                }
                assert!(dest.join("export/empty").is_dir());
            }
        });
}
