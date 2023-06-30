use std::{fs, path::PathBuf, time::Duration};

use drive_lib::Drive;

fn new_test_dir(name: &str) -> PathBuf {
    let dir = format!("/tmp/{name}");
    drop(fs::remove_dir_all(&dir));
    fs::create_dir_all(&dir).expect("failed to create test directory");
    PathBuf::from(&dir)
}

#[test]
fn new_file() {
    let mut dest = new_test_dir("new_file");
    let drive = Drive::test_drive();

    let drive2 = drive.clone();
    let dest_clone = dest.clone();
    std::thread::spawn(move || {
        drive2.check_for_changes(dest_clone);
    });
    std::thread::sleep(Duration::from_secs(3));

    dest = drive.prep_destination(dest);
    dest.push("test.md");
    fs::File::create(&dest).unwrap();

    println!("{:#?}", drive.pending_events.lock().unwrap());
    assert_eq!(drive.pending_events.lock().unwrap().len(), 1);
}
