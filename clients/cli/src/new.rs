use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::{fs, io};

use uuid::Uuid;

use lockbook_core::client::{Client, CreateFileError, CreateFileRequest};

use lockbook_core::repo::account_repo::AccountRepo;

use crate::{connect_to_db, get_editor};
use lockbook_core::model::file_metadata::{FileMetadata, Status};
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::auth_service::AuthService;
use lockbook_core::service::file_service::{FileService, NewFileError, UpdateFileError};
use lockbook_core::{
    DefaultAccountRepo, DefaultAuthService, DefaultClient, DefaultFileMetadataRepo,
    DefaultFileService,
};

pub fn new() {
    let db = connect_to_db();
    let account =
        DefaultAccountRepo::get_account(&db).expect("No account found, run init, import or help.");

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
    println!("Creating file {}", &file_name);
    file_name.retain(|c| !c.is_whitespace());

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

    let edit_was_successful = Command::new(get_editor())
        .arg(&file_location)
        .spawn()
        .expect(
            format!(
                "Failed to spawn: {}, content location: {}",
                get_editor(),
                &file_location
            )
            .as_str(),
        )
        .wait()
        .expect(
            format!(
                "Failed to wait for spawned process: {}, content location: {}",
                get_editor(),
                &file_location
            )
            .as_str(),
        )
        .success();

    if edit_was_successful {
        let file_content =
            fs::read_to_string(temp_file_path).expect("Could not read file that was edited");

        let encrypted_file = match DefaultFileService::update(&db, &file_metadata.id, &file_content)
        {
            Ok(file) => file,
            Err(err) => match err {
                UpdateFileError::AccountRetrievalError(_) => panic!(
                    "No account found, run init, import, or help, aborting without cleaning up"
                ),
                UpdateFileError::FileRetrievalError(_) => {
                    panic!("Failed to get file being edited, aborting without cleaning up")
                }
                UpdateFileError::EncryptedWriteError(_) => {
                    panic!("Failed to perform encryption!, aborting without cleaning up")
                }
                UpdateFileError::MetadataDbError(_) => {
                    panic!("Failed to update file metadata, aborting without cleaning up")
                }
            },
        };

        match DefaultClient::create_file(&CreateFileRequest {
            username: account.username.clone(),
            auth: DefaultAuthService::generate_auth(&account).expect("Failed to sign message"),
            file_id: file_metadata.id.clone(),
            file_name: file_metadata.name.clone(),
            file_path: file_location.clone(),
            file_content: serde_json::to_string(&encrypted_file)
                .expect("Failed to serialize encrypted file"),
        }) {
            Ok(version) => {
                DefaultFileMetadataRepo::update(
                    &db,
                    &FileMetadata {
                        id: file_metadata.id,
                        name: file_metadata.name,
                        path: file_metadata.path,
                        updated_at: 0,
                        version,
                        status: Status::Synced,
                    },
                )
                .expect("Failed to update metadata repo");
                print!("File saved locally and synced!")
            }
            Err(err) => match err {
                CreateFileError::SendFailed(_) => {
                    eprintln!("Network error occurred, file will be sent next sync")
                }
                _ => eprint!("Unknown error occurred sending file, file exists locally."),
            },
        }
    } else {
        eprintln!(
            "{} indicated a problem, aborting and cleaning up",
            get_editor()
        );
    }

    fs::remove_file(&temp_file_path)
        .expect(format!("Failed to delete temporary file: {}", &file_location).as_str());
}
