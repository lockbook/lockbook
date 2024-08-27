use lb_rs::Core;
use std::thread;
use std::time::{Duration, SystemTime};
use test_utils::{random_name, test_config, test_core_with_account, url};

#[test]
#[ignore]
fn test_sync_concurrently() {
    println!("test began");
    let core = test_core_with_account();
    for i in 0..100 {
        let file = core.create_at_path(&format!("{i}")).unwrap();
        core.write_document(file.id, "t".repeat(1000).as_bytes())
            .unwrap();
    }
    core.sync(None).unwrap();

    // 1779, 1942
    let core1 = Core::init(&test_config()).unwrap();
    let core2 = core1.clone();
    core1
        .import_account(&core.export_account_string().unwrap())
        .unwrap();
    let th1 = thread::spawn(move || {
        println!("in th1");
        let start = SystemTime::now();
        core1.sync(None).unwrap();
        SystemTime::now().duration_since(start).unwrap().as_millis()
    });

    for _ in 0..100 {
        core2.get_uncompressed_usage().unwrap();
        thread::sleep(Duration::from_millis(1));
    }

    let th1 = th1.join().unwrap();

    println!("{th1}");
}

#[test]
#[ignore]
fn test_sync_concurrently2() {
    println!("test began");
    let core = test_core_with_account();
    let file = core.create_at_path("test.md").unwrap();
    core.write_document(file.id, "t".repeat(1000).as_bytes())
        .unwrap();
    core.sync(None).unwrap();

    let mut threads = vec![];

    for _ in 0..1 {
        let core1 = core.clone();
        let th1 = thread::spawn(move || {
            println!("in th1");
            let start = SystemTime::now();
            for _ in 0..10 {
                println!("sync began");
                if let Err(e) = core1.sync(None) {
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
        let th2 = thread::spawn(move || {
            println!("in th2");
            let start = SystemTime::now();
            for i in 0..100 {
                println!("write began");
                if let Err(e) = core2.write_document(file.id, i.to_string().repeat(x).as_bytes()) {
                    eprintln!("ERROR FOUND: {e}");
                }
                println!("write end");
            }
            SystemTime::now().duration_since(start).unwrap().as_millis()
        });
        threads.push(th2);
    }

    threads.into_iter().for_each(|th| {
        th.join().unwrap();
    });
}

#[test]
fn new_account_test() {
    let core = Core::init(&test_config()).unwrap();
    core.create_account(&random_name(), &url(), false).unwrap();
    assert_eq!(core.calculate_work().unwrap().work_units, vec![]);
}
