use lb_rs::Lb;
use std::future::IntoFuture;
use std::thread;
use test_utils::{random_name, test_config, test_core_with_account, url};
use web_time::{Duration, SystemTime};

#[tokio::test]
#[ignore]
async fn test_sync_concurrently() {
    println!("test began");
    let core = test_core_with_account().await;
    for i in 0..100 {
        let file = core.create_at_path(&format!("{i}")).await.unwrap();
        core.write_document(file.id, "t".repeat(1000).as_bytes())
            .await
            .unwrap();
    }
    core.sync(None).await.unwrap();

    // 1779, 1942
    let core1 = Lb::init(test_config()).await.unwrap();
    let core2 = core1.clone();
    core1
        .import_account(&core.export_account_private_key().unwrap(), Some(&url()))
        .await
        .unwrap();
    let th1 = tokio::spawn(async move {
        println!("in th1");
        let start = SystemTime::now();
        core1.sync(None).await.unwrap();
        SystemTime::now().duration_since(start).unwrap().as_millis()
    });

    for _ in 0..100 {
        core2.get_uncompressed_usage().await.unwrap();
        thread::sleep(Duration::from_millis(1));
    }

    let th1 = th1.into_future().await.unwrap();

    println!("{th1}");
}

#[tokio::test]
#[ignore]
async fn test_sync_concurrently2() {
    println!("test began");
    let core = test_core_with_account().await;
    let file = core.create_at_path("test.md").await.unwrap();
    core.write_document(file.id, "t".repeat(1000).as_bytes())
        .await
        .unwrap();
    core.sync(None).await.unwrap();

    let mut threads = vec![];

    for _ in 0..1 {
        let core1 = core.clone();
        let th1 = tokio::spawn(async move {
            println!("in th1");
            let start = SystemTime::now();
            for _ in 0..10 {
                println!("sync began");
                if let Err(e) = core1.sync(None).await {
                    eprintln!("ERROR FOUND: {e}");
                }
                println!("sync end");
            }
            SystemTime::now().duration_since(start).unwrap().as_millis()
        });
        threads.push(th1);
    }

    for x in 0..100 {
        let core2 = core.clone();
        let th2 = tokio::spawn(async move {
            println!("in th2");
            let start = SystemTime::now();
            for i in 0..100 {
                println!("write began");
                if let Err(e) = core2
                    .write_document(file.id, i.to_string().repeat(x).as_bytes())
                    .await
                {
                    eprintln!("ERROR FOUND: {e}");
                }
                println!("write end");
            }
            SystemTime::now().duration_since(start).unwrap().as_millis()
        });
        threads.push(th2);
    }

    for th in threads {
        th.into_future().await.unwrap();
    }
}

#[tokio::test]
async fn new_account_test() {
    let core = Lb::init(test_config()).await.unwrap();
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    assert_eq!(core.calculate_work().await.unwrap().work_units, vec![]);
}
