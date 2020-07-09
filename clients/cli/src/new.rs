use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::{fs, io};

use uuid::Uuid;

use lockbook_core::model::crypto::DecryptedValue;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::file_service::{FileService, NewFileFromPathError};
use lockbook_core::{DefaultFileMetadataRepo, DefaultFileService};

use crate::utils::{connect_to_db, edit_file_with_editor, get_account};

pub fn new() {
    get_account(&connect_to_db());

    let file_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    let temp_file_path = Path::new(file_location.as_str());
    File::create(&temp_file_path)
        .unwrap_or_else(|_| panic!("Could not create temporary file: {}", &file_location));

    print!("Enter a filepath: ");
    io::stdout().flush().unwrap();

    let mut file_name = String::new();
    io::stdin()
        .read_line(&mut file_name)
        .expect("Failed to read from stdin");
    file_name.retain(|c| !c.is_whitespace());

    let file_metadata = match DefaultFileService::create_at_path(&connect_to_db(), &file_name) {
        Ok(file_metadata) => file_metadata,
        Err(error) => match error {
            NewFileFromPathError::InvalidRootFolder => panic!("The first component of your file path does not match the name of your root folder!"),
            NewFileFromPathError::DbError(_) |
            NewFileFromPathError::NoRoot | NewFileFromPathError::FailedToRecordChange(_) |
            NewFileFromPathError::FailedToCreateChild(_) => panic!("Unexpected error ocurred: {:?}", error),
        },
    };

    let edit_was_successful = edit_file_with_editor(&file_location);

    if edit_was_successful {
        let secret =
            fs::read_to_string(temp_file_path).expect("Could not read file that was edited");

        DefaultFileService::write_document(&connect_to_db(), file_metadata.id, &DecryptedValue { secret })
            .expect("Unexpected error while updating internal state");

        DefaultFileMetadataRepo::insert(&connect_to_db(), &file_metadata).expect("Failed to index new file!");
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path)
        .unwrap_or_else(|_| panic!("Failed to delete temporary file: {}", &file_location));
}
