use std::{fs, path::PathBuf, time::Duration};

use drive_lib::{Drive, event::DriveEvent};

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
    std::thread::sleep(Duration::from_millis(500));

    dest = drive.get_dest();
    dest.push("test.md");
    println!("{:?}", dest);
    fs::File::create(&dest).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    println!("{:#?}", drive.pending_events.lock().unwrap());
    assert_eq!(drive.pending_events.lock().unwrap().len(), 1);
    assert_eq!(drive.pending_events.lock().unwrap()[0], DriveEvent::Create("test.md".to_string()));
}

#[test]
fn delete_file(){
    let mut dest = new_test_dir("new_file");
    let drive = Drive::test_drive();

    let drive2 = drive.clone();
    let dest_clone = dest.clone();
    std::thread::spawn(move || {
        drive2.check_for_changes(dest_clone);
    });
    std::thread::sleep(Duration::from_millis(500));

    dest = drive.get_dest();
    dest.push("test.md");
    println!("{:?}", dest);
    fs::File::create(&dest).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    fs::remove_file(&dest).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    println!("{:#?}", drive.pending_events.lock().unwrap());
    assert_eq!(drive.pending_events.lock().unwrap().len(), 2);
}
