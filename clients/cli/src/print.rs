use std::io;
use std::io::Write;

use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::file_service::FileService;
use lockbook_core::{DefaultFileMetadataRepo, DefaultFileService};

use crate::utils::{connect_to_db, get_account};

pub fn print() {
    let db = connect_to_db();
    get_account(&db);

    if atty::is(atty::Stream::Stdout) {
        print!("Enter a filepath: ");
    }

    io::stdout().flush().unwrap();
    let mut file_name = String::new();
    io::stdin()
        .read_line(&mut file_name)
        .expect("Failed to read from stdin");
    file_name.retain(|c| !c.is_whitespace());

    let file_metadata = DefaultFileMetadataRepo::get_by_path(&db, &file_name)
        .expect("Could not search files ")
        .expect("Could not find that file!");

    match DefaultFileService::read_document(&db, file_metadata.id) {
        Ok(content) => print!("{}", content.secret),
        Err(error) => panic!("Unexpected error: {:?}", error),
    };
}
