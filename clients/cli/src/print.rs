use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::file_service::FileService;
use lockbook_core::{DefaultFileMetadataRepo, DefaultFileService};

use crate::utils::{connect_to_db, get_account};

pub fn print(file_name: &str) {
    get_account(&connect_to_db());

    let file_metadata = DefaultFileMetadataRepo::get_by_path(&connect_to_db(), &file_name)
        .expect("Could not search files ")
        .expect("Could not find that file!");

    match DefaultFileService::read_document(&connect_to_db(), file_metadata.id) {
        Ok(content) => print!("{}", content.secret),
        Err(error) => panic!("Unexpected error: {:?}", error),
    };
}
