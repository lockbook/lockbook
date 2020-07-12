use crate::utils::connect_to_db;
use lockbook_core::model::file_metadata::FileMetadata;
use lockbook_core::model::file_metadata::FileType::Folder;
use lockbook_core::repo::file_metadata_repo::{DbError, FileMetadataRepo};
use lockbook_core::service::file_service::{DocumentMoveError, FileService};
use lockbook_core::{DefaultFileMetadataRepo, DefaultFileService};

pub fn move_file(path1: &str, path2: &str) {
    let db = connect_to_db();

    match DefaultFileMetadataRepo::get_by_path(&db, path1) {
        Ok(maybe_old_file) => match maybe_old_file {
            None => eprintln!("No file found at {}", path1),
            Some(file1) => match DefaultFileMetadataRepo::get_by_path(&db, path2) {
                Ok(maybe_dest) => match maybe_dest {
                    Some(destination) => {
                        if destination.file_type == Folder {
                            match DefaultFileService::move_file(&db, file1.id, destination.id) {
                                Ok(_) => {}
                                Err(err) => match err {
                                    DocumentMoveError::TargetParentHasChildNamedThat => {
                                        eprintln!("{} has a child named {}", path2, file1.name)
                                    }
                                    DocumentMoveError::FileDoesntExist
                                    | DocumentMoveError::AccountRetrievalError(_)
                                    | DocumentMoveError::NewParentDoesntExist
                                    | DocumentMoveError::DbError(_)
                                    | DocumentMoveError::FailedToRecordChange(_)
                                    | DocumentMoveError::FailedToDecryptKey(_)
                                    | DocumentMoveError::FailedToReEncryptKey(_)
                                    | DocumentMoveError::CouldNotFindParents(_) => {
                                        eprintln!("Unexpected error: {:#?}", err)
                                    }
                                },
                            }
                        } else {
                            eprintln!("{} is a document", path2)
                        }
                    }
                    None => eprintln!("No file found at {}", path2),
                },
                Err(err) => eprintln!("Unexpected error: {:#?}", err),
            },
        },
        Err(err) => eprintln!("Unexpected error: {:#?}", err),
    }
}
