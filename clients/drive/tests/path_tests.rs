use std::path::PathBuf;

use drive_lib::local_sync::get_lockbook_path;

#[test]
fn path_test() {
    let dest = PathBuf::from("/folder/username/");
    let evt = PathBuf::from("/folder/username/event.md");

    assert_eq!(get_lockbook_path(evt, dest), PathBuf::from("event.md"))
}
