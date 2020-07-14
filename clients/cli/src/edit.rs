use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_core::model::crypto::DecryptedValue;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::file_service::FileService;
use lockbook_core::{DefaultFileMetadataRepo, DefaultFileService};

use crate::utils::{connect_to_db, edit_file_with_editor, get_account};

pub fn edit(file_name: &str) {
    get_account(&connect_to_db());

    let file_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    let temp_file_path = Path::new(file_location.as_str());
    let mut file_handle = File::create(&temp_file_path)
        .expect(format!("Could not create temporary file: {}", &file_location).as_str());

    let file_metadata = DefaultFileMetadataRepo::get_by_path(&connect_to_db(), &file_name)
        .expect("Could not search files ")
        .expect("Could not find that file!");

    let file_content = match DefaultFileService::read_document(&connect_to_db(), file_metadata.id) {
        Ok(content) => content,
        Err(error) => panic!("Unexpected error: {:?}", error),
    };

    file_handle
        .write_all(&file_content.secret.into_bytes())
        .unwrap_or_else(|_| {
            panic!(
                "Failed to write decrypted contents to temporary file, check {}",
                file_location
            )
        });

    file_handle.sync_all().unwrap_or_else(|_| {
        panic!(
            "Failed to write decrypted contents to temporary file, check {}",
            file_location
        )
    });

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
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path)
        .expect(format!("Failed to delete temporary file: {}", &file_location).as_str());
}
