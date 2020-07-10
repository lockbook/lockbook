use std::io;
use std::io::Write;
use std::process::exit;

use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::DefaultFileMetadataRepo;

use crate::utils::{connect_to_db, get_account};

pub fn remove(file_name: &str) {
    get_account(&connect_to_db());
    let mut file_metadata = DefaultFileMetadataRepo::get_by_path(&connect_to_db(), &file_name)
        .expect("Could not search files ")
        .expect("Could not find that file!");

    file_metadata.deleted = true;

    DefaultFileMetadataRepo::insert(&connect_to_db(), &file_metadata).unwrap_or_else(|err| {
        eprintln!("Unexpected error occurred: {:?}", err);
        exit(1)
    });

    println!("File marked for deletion");
}
