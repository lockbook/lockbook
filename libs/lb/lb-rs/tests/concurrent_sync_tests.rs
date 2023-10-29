use lb_rs::Core;
use std::thread;
use std::time::{Duration, SystemTime};
use test_utils::{test_config, test_core_with_account};

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
        .import_account(&core.export_account().unwrap())
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
