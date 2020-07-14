use std::fs;
use std::fs::File;
use std::path::Path;

use lockbook_core::model::crypto::DecryptedValue;
use lockbook_core::model::file_metadata::FileType::Folder;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::file_service::{FileService, NewFileFromPathError};
use lockbook_core::{get_account, DefaultFileMetadataRepo, DefaultFileService, GetAccountError};
use uuid::Uuid;

use crate::utils::{connect_to_db, edit_file_with_editor, exit_with, get_config};
use crate::{NO_ACCOUNT, UNEXPECTED_ERROR};
use std::process::exit;

pub fn new(file_name: &str) {
    match get_account(&get_config()) {
        Ok(_) => {}
        Err(err) => match err {
            GetAccountError::NoAccount => {
                exit_with("No account! Run init or import to get started.", NO_ACCOUNT)
            }
            GetAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }

    let file_location = format!("/tmp/{}/{}", Uuid::new_v4().to_string(), file_name);
    let temp_file_path = Path::new(file_location.as_str());
    match File::create(&temp_file_path) {
        Ok(_) => {}
        Err(err) => exit_with(
            &format!("Could not open temporary file for writing. OS: {:#?}", err),
            UNEXPECTED_ERROR,
        ),
    }

    let file_metadata = match DefaultFileService::create_at_path(&connect_to_db(), &file_name) {
        Ok(file_metadata) => file_metadata,
        Err(error) => match error {
            NewFileFromPathError::InvalidRootFolder => panic!("The first component of your file path does not match the name of your root folder!"),
            NewFileFromPathError::DbError(_) |
            NewFileFromPathError::NoRoot | NewFileFromPathError::FailedToRecordChange(_) |
            NewFileFromPathError::FailedToCreateChild(_) => panic!("Unexpected error ocurred: {:?}", error),
            NewFileFromPathError::FileAlreadyExists => panic!("This file already exists")
        },
    };

    if file_metadata.file_type == Folder {
        eprintln!("This is a folder"); // TODO why are we upset about this?
        exit(1);
    }

    let edit_was_successful = edit_file_with_editor(&file_location);

    if edit_was_successful {
        let secret =
            fs::read_to_string(temp_file_path).expect("Could not read file that was edited");

        DefaultFileService::write_document(
            &connect_to_db(),
            file_metadata.id,
            &DecryptedValue { secret },
        )
        .expect("Unexpected error while updating internal state");

        DefaultFileMetadataRepo::insert(&connect_to_db(), &file_metadata)
            .expect("Failed to index new file!");
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path)
        .unwrap_or_else(|_| panic!("Failed to delete temporary file: {}", &file_location));
}
