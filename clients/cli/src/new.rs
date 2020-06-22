use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::{fs, io};

use uuid::Uuid;

use crate::utils::{connect_to_db, edit_file_with_editor, get_account, get_editor};

use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;

use lockbook_core::service::file_service::{FileService, NewFileError};

use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{DefaultFileMetadataRepo, DefaultFileService, DefaultSyncService};

pub fn new() {
    let db = connect_to_db();
    get_account(&db);

    let file_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    let temp_file_path = Path::new(file_location.as_str());
    File::create(&temp_file_path)
        .expect(format!("Could not create temporary file: {}", &file_location).as_str());

    print!("Enter a filename: ");
    io::stdout().flush().unwrap();

    let mut file_name = String::new();
    io::stdin()
        .read_line(&mut file_name)
        .expect("Failed to read from stdin");
    file_name.retain(|c| !c.is_whitespace());
    println!("Creating file {}", &file_name);

    let file_metadata = match DefaultFileService::create(&db, &file_name, &file_location) {
        Ok(file_metadata) => file_metadata,
        Err(error) => match error {
            NewFileError::AccountRetrievalError(_) => {
                panic!("No account found, run init, import, or help.")
            }
            NewFileError::EncryptedFileError(_) => panic!("Failed to perform encryption!"),
            NewFileError::SavingMetadataFailed(_) => {
                panic!("Failed to persist file metadata locally")
            }
            NewFileError::SavingFileContentsFailed(_) => {
                panic!("Failed to persist file contents locally")
            }
        },
    };

    let edit_was_successful = edit_file_with_editor(&file_location);

    if edit_was_successful {
        let file_content =
            fs::read_to_string(temp_file_path).expect("Could not read file that was edited");

        DefaultFileService::write_document(&db, &file_metadata.id, &file_content)
            .expect("Unexpected error while updating internal state");

        println!("Updating local state.");
        DefaultFileMetadataRepo::insert(&db, &file_metadata).expect("Failed to index new file!");

        println!("Syncing");
        DefaultSyncService::sync(&db).expect("Failed to sync");

        println!("Sync successful, cleaning up.")
    } else {
        eprintln!(
            "{} indicated a problem, aborting and cleaning up",
            get_editor()
        );
    }

    fs::remove_file(&temp_file_path)
        .expect(format!("Failed to delete temporary file: {}", &file_location).as_str());
}
