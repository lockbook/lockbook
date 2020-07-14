use std::fs;
use std::path::PathBuf;
use std::process::exit;

use lockbook_core::model::crypto::DecryptedValue;
use lockbook_core::service::file_service::{FileService, NewFileFromPathError};
use lockbook_core::DefaultFileService;

use crate::utils::{connect_to_db, get_account};

pub fn copy(path: PathBuf) {
    let metadata = fs::metadata(&path).unwrap_or_else(|err| {
        eprintln!("Failed to read file metadata: {}", err);
        exit(1);
    });

    if metadata.is_file() {
        let secret = fs::read_to_string(&path).unwrap_or_else(|err| {
            eprintln!("Failed to read file: {}", err);
            exit(2);
        });

        let account = get_account(&connect_to_db());

        let absolute_path_maybe = fs::canonicalize(&path).unwrap_or_else(|error| {
            eprintln!("Failed to get absolute path: {}", error);
            exit(3);
        });

        let absolute_path_string = absolute_path_maybe.to_str().unwrap_or_else(|| {
            eprintln!("Absolute path not a valid utf-8 sequence!");
            exit(4);
        });

        let file_metadata = DefaultFileService::create_at_path(
            &connect_to_db(),
            format!(
                "{}/imported/cli-copy{}",
                account.username, absolute_path_string
            )
            .as_str(),
        )
        .unwrap_or_else(|err| match err {
            NewFileFromPathError::NoRoot => {
                eprintln!("Account missing root, has a sync been performed?");
                exit(5);
            }
            _ => {
                eprintln!("Unexpected error occurred: {:?}", err);
                exit(6)
            }
        });

        DefaultFileService::write_document(
            &connect_to_db(),
            file_metadata.id,
            &DecryptedValue { secret },
        )
        .unwrap_or_else(|error| {
            eprintln!("Unexpected error while saving file contents: {:?}", error);
            exit(7);
        });

        if atty::is(atty::Stream::Stdout) {
            println!("{} saved", file_metadata.name);
        } else {
            println!("{}", file_metadata.name);
        }
    } else {
        unimplemented!("Folders are not supported yet!")
    }
}
