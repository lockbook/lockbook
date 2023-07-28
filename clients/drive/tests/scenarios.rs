use std::{sync::Arc, thread, fs, time::Duration, path::PathBuf};
use drive_lib::Drive;

fn new_test_dir(name: &str) -> PathBuf {
    let dir = format!("/tmp/{name}");
    drop(fs::remove_dir_all(&dir));
    fs::create_dir_all(&dir).expect("failed to create test directory");
    PathBuf::from(&dir)
}

#[test]
fn create_file_test(){

    let dest = new_test_dir("new_file");

    let drive = Drive::test_drive();

    let core = drive.c.clone();

    let clone_drive = drive.clone();
    let dest_clone = dest.clone();

    thread::spawn(move ||clone_drive.check_for_changes(dest_clone));
    thread::sleep(Duration::from_millis(1500));

    let mut dest = drive.get_dest();
    dest.push("test.md");

    fs::File::create(&dest).unwrap();
    thread::sleep(Duration::from_millis(1500));

    let files = core.list_metadatas().unwrap();
    println!("{:?}", files.iter().find(|m|m.name == "test.md"));
    let f = files.iter().find(|m|m.name == "test.md").unwrap();
    assert_eq!(f.parent, core.get_root().unwrap().id);
}

//Make create_file_test pass
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
