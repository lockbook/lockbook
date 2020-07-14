use crate::utils::{connect_to_db, get_account};
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::file_service::{DocumentRenameError, FileService};
use lockbook_core::{DefaultFileMetadataRepo, DefaultFileService};

pub fn rename(path: &str, new_name: &str) {
    let db = connect_to_db();
    get_account(&db);

    match DefaultFileMetadataRepo::get_by_path(&db, path) {
        Ok(maybe_fm) => match maybe_fm {
            None => {
                eprintln!("That path does not exist!");
            }
            Some(fm) => match DefaultFileService::rename_file(&db, fm.id, new_name) {
                Ok(_) => {
                    db.flush().unwrap();
                }
                Err(rename_error) => match rename_error {
                    DocumentRenameError::FileNameContainsSlash => {
                        eprintln!("The new name cannot contain a slash.");
                    }
                    DocumentRenameError::FileNameNotAvailable => {
                        eprintln!("A file with this name exists at this location already.");
                    }
                    DocumentRenameError::FileDoesNotExist
                    | DocumentRenameError::DbError(_)
                    | DocumentRenameError::FailedToRecordChange(_) => {
                        eprintln!("An unexpected error occurred! {:#?}", rename_error);
                    }
                },
            },
        },
        Err(err) => {
            eprintln!("An unexpected error occurred! {:#?}", err);
        }
    }
}
