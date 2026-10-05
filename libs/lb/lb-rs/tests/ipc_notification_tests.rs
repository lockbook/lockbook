#![cfg(all(unix, not(target_os = "ios")))]

use std::env;
use std::io::{self, Read};
use std::process::{self, Stdio};
use std::time::Duration;

use db_rs::View;
use lb_rs::Lb;
use lb_rs::service::events::Event;
use test_utils::{test_config, test_core};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::runtime::Builder;
use tokio::time::timeout;

#[tokio::test]
async fn socket_notifications_catch_up_without_a_caller_transaction() {
    let writer = test_core().await;
    let mut config = writer.config.clone();
    config.background_work = true;
    let reader = Lb::init(config.clone()).await.unwrap();
    let second_reader = Lb::init(config).await.unwrap();
    // Keep periodic sync from doing the catch-up we're expecting from the socket.
    let _sync = reader.syncer.lock().await;
    let _second_sync = second_reader.syncer.lock().await;
    let mut events = reader.subscribe();
    let mut second_events = second_reader.subscribe();

    for last_synced in [123, 456] {
        let mut tx = writer.begin_tx().await;
        tx.db().last_synced.replace(last_synced).unwrap();
        if last_synced == 123 {
            tx.end();
        } else {
            drop(tx);
        }

        for events in [&mut events, &mut second_events] {
            timeout(Duration::from_secs(5), async {
                while !matches!(events.recv().await.unwrap(), Event::IpcChangesApplied) {}
            })
            .await
            .expect("the socket notification did not trigger catch-up");
        }
        assert_eq!(reader.ro_tx().await.db().last_synced.as_ref(), Some(&last_synced));
        assert_eq!(second_reader.ro_tx().await.db().last_synced.as_ref(), Some(&last_synced));

        // Notifications must still work after the writer switches to a new log.
        writer.db.write().await.snapshot().unwrap();
    }
}

#[tokio::test]
async fn a_writer_process_notifies_before_closing_its_log_and_before_exit() {
    let mut config = test_config();
    config.background_work = true;
    let reader = Lb::init(config.clone()).await.unwrap();
    let _sync = reader.syncer.lock().await;
    let mut events = reader.subscribe();
    let mut writer = Command::new(env::current_exe().unwrap())
        .args(["--exact", "ipc_writer_process", "--ignored"])
        .env("LB_IPC_TEST_PATH", &config.writeable_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();

    for expected in [123, 456] {
        timeout(Duration::from_secs(5), async {
            while !matches!(events.recv().await.unwrap(), Event::IpcChangesApplied) {}
        })
        .await
        .expect("the writer process did not wake the reader");
        assert_eq!(reader.ro_tx().await.db().last_synced.as_ref(), Some(&expected));
        if expected == 123 {
            assert!(writer.try_wait().unwrap().is_none());
            writer
                .stdin
                .as_mut()
                .unwrap()
                .write_all(&[1])
                .await
                .unwrap();
        }
    }
    assert!(
        timeout(Duration::from_secs(5), writer.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[test]
#[ignore = "subprocess helper for the IPC test"]
fn ipc_writer_process() {
    let mut config = test_config();
    config.writeable_path = env::var("LB_IPC_TEST_PATH").unwrap();
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let writer = Lb::init(config).await.unwrap();
            let mut tx = writer.begin_tx().await;
            tx.db().last_synced.replace(123).unwrap();
            tx.end();

            // Keep the log open until the reader has received the first notification.
            io::stdin().read_exact(&mut [0]).unwrap();
            writer.db.write().await.snapshot().unwrap();
            let mut tx = writer.begin_tx().await;
            tx.db().last_synced.replace(456).unwrap();
            drop(tx);
            // A CLI must not need a background task to run before exiting.
            process::exit(0);
        });
}
