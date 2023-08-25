use drive_lib::Drive;
use std::{fs, path::PathBuf, sync::Arc, thread, time::Duration};

fn new_test_dir(name: &str) -> PathBuf {
    let dir = format!("/tmp/{name}");
    drop(fs::remove_dir_all(&dir));
    fs::create_dir_all(&dir).expect("failed to create test directory");
    PathBuf::from(&dir)
}

#[test]
fn create_file_test() {
    let dest = new_test_dir("new_file");

    let drive = Drive::test_drive();

    let core = drive.c.clone();

    let clone_drive = drive.clone();
    let dest_clone = dest.clone();

    thread::spawn(move || clone_drive.check_for_changes(dest_clone));
    thread::sleep(Duration::from_millis(100));

    let mut dest = drive.get_dest();
    dest.push("test.md");

    fs::File::create(&dest).unwrap();
    thread::sleep(Duration::from_millis(100));

    let files = core.list_metadatas().unwrap();
    println!("{:#?}", files.iter());
    println!("{:?}", files.iter().find(|m| m.name == "test.md"));
    let f = files.iter().find(|m| m.name == "test.md").unwrap();
    assert_eq!(f.parent, core.get_root().unwrap().id);
    assert_eq!(files.len(), 2);
}

#[test]
fn rename_file_test() {
    let dest = new_test_dir("new_file");

    let drive = Drive::test_drive();

    let core = drive.c.clone();

    let clone_drive = drive.clone();
    let dest_clone = dest.clone();

    thread::spawn(move || clone_drive.check_for_changes(dest_clone));
    thread::sleep(Duration::from_millis(100));

    let mut dest = drive.get_dest();
    let mut dest2 = dest.clone();
    dest.push("test.md");
    dest2.push("test2.md");

    fs::File::create(&dest).unwrap();
    thread::sleep(Duration::from_millis(100));

    fs::rename(dest, dest2).unwrap();
    thread::sleep(Duration::from_millis(100));

    let files = core.list_metadatas().unwrap();
    println!("{:#?}", files.iter());
    println!("{:?}", files.iter().find(|m| m.name == "test2.md"));
    let f = files.iter().find(|m| m.name == "test2.md").unwrap();
    assert_eq!(f.parent, core.get_root().unwrap().id);
    assert_eq!(files.len(), 2);
}

#[test]
fn move_file_test() {
    let dest = new_test_dir("new_file");

    let drive = Drive::test_drive();

    let core = drive.c.clone();

    let clone_drive = drive.clone();
    let dest_clone = dest.clone();

    thread::spawn(move || clone_drive.check_for_changes(dest_clone));
    thread::sleep(Duration::from_millis(100));

    let mut dest = drive.get_dest();
    let mut dest2 = dest.clone();
    dest.push("example/test.md");
    dest2.push("example2/test.md");

    let mut folder1 = drive.get_dest();
    let mut folder2 = folder1.clone();
    folder1.push("example/");
    folder2.push("example2/");

    fs::create_dir(folder1).unwrap();
    fs::create_dir(folder2).unwrap();
    fs::File::create(&dest).unwrap();
    thread::sleep(Duration::from_millis(1000));
    //panic!();

    fs::rename(dest, dest2).unwrap();
    thread::sleep(Duration::from_millis(1000));

    let files = core.list_metadatas().unwrap();
    println!("{:#?}", files.iter());
    println!("{:?}", files.iter().find(|m| m.name == "test.md"));
    let f = files.iter().find(|m| m.name == "test.md").unwrap();
    let folder = files.iter().find(|m| m.name == "example2").unwrap();
    assert_eq!(f.parent, folder.id);
    assert_eq!(files.len(), 4);
}

#[test]
fn remove_file_test(){
    let dest = new_test_dir("new_file");

    let drive = Drive::test_drive();

    let core = drive.c.clone();

    let clone_drive = drive.clone();
    let dest_clone = dest.clone();

    thread::spawn(move || clone_drive.check_for_changes(dest_clone));
    thread::sleep(Duration::from_millis(100));

    let mut dest = drive.get_dest();
    dest.push("test.md");

    fs::File::create(&dest).unwrap();
    thread::sleep(Duration::from_millis(100));

    let filesprev = core.list_metadatas().unwrap();
    thread::sleep(Duration::from_millis(100));

    fs::remove_file(&dest).unwrap();
    thread::sleep(Duration::from_millis(100));

    let filesnew = core.list_metadatas().unwrap();
    println!("{:#?}", filesprev.iter());
    println!("{:#?}", filesnew.iter());
    let f = filesnew.iter().any(|m| m.name == "test.md");
    assert_eq!(f, false);
    assert_eq!(filesprev.len(), 2);
    assert_eq!(filesnew.len(), 1);
}
//Make create_file_test pass -- you're creating an extra folder inside root
//Write more tests (ideally look at code and write test for each branch)
//As soon as reliable, update on it
//Work on sync stuff
//Write a function that takes two lists of files and tells exactly what happened
//enum variables ->create, rename, write_contents, etc
//fn file_diff(a: Vec<File>, b: Vec<File>) -> Vec<Diff>{}
/*pub enum Diff{
    Create_File
    Rename_File, etc
}*/
