#![cfg(not(target_family = "wasm"))]

use lb_rs::Lb;
use std::{sync::mpsc, thread, time::Duration};
use test_utils::test_config;
use tokio::runtime::Builder;

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
