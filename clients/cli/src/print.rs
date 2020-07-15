use crate::utils::{connect_to_db, exit_with, exit_with_no_account, get_config};
use crate::UNEXPECTED_ERROR;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::file_service::FileService;
use lockbook_core::{get_account, DefaultFileMetadataRepo, DefaultFileService, GetAccountError};

pub fn print(file_name: &str) {
    match get_account(&get_config()) {
        Ok(_) => {}
        Err(err) => match err {
            GetAccountError::NoAccount => exit_with_no_account(),
            GetAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }

    let file_metadata = DefaultFileMetadataRepo::get_by_path(&connect_to_db(), &file_name)
        .expect("Could not search files ")
        .expect("Could not find that file!");

    match DefaultFileService::read_document(&connect_to_db(), file_metadata.id) {
        Ok(content) => print!("{}", content.secret),
        Err(error) => panic!("Unexpected error: {:?}", error),
    };
}
