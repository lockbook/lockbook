#![cfg(not(target_family = "wasm"))]

use db_rs::View;
use lb_rs::Lb;
use std::{sync::mpsc, thread, time::Duration};
use test_utils::test_config;
use tokio::runtime::Builder;

#[test]
fn reused_blocking_core_refreshes_between_concurrent_requests() {
    let config = test_config();
    let reader = lb_rs::blocking::Lb::init(config.clone()).unwrap();
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let writer = runtime.block_on(Lb::init(config)).unwrap();

    for (previous, current) in [(0, 123), (123, 456)] {
        runtime.block_on(async {
            let mut tx = writer.begin_tx().await;
            tx.db().last_synced.replace(current).unwrap();
            tx.end();
        });
        assert_eq!(reader.get_last_synced().unwrap(), previous);

        thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    reader.refresh();
                    assert_eq!(reader.get_last_synced().unwrap(), current);
                });
            }
        });
        // The same reader must also catch up after the writer rotates its log.
        runtime.block_on(async { writer.db.write().await.snapshot().unwrap() });
    }
}

#[test]
#[ignore = "independent instances can deadlock a shared Tokio runtime"]
fn contended_transactions_do_not_block_the_runtime() {
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let config = test_config();
                let first = Lb::init(config.clone()).await.unwrap();
                let second = Lb::init(config).await.unwrap();
                let tx = first.begin_tx().await;

                tokio::join!(
                    biased;
                    async { second.begin_tx().await.end() },
                    async { tx.end() },
                );
            });
        done.send(()).unwrap();
    });

    // A Tokio timeout cannot fire if the runtime itself is blocked.
    finished
        .recv_timeout(Duration::from_secs(5))
        .expect("contended begin_tx blocked the runtime from releasing the first transaction");
}
