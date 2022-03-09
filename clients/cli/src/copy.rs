use std::cell::Cell;
use std::io::Write;
use std::path::PathBuf;

use lockbook_core::service::import_export_service::ImportStatus;
use lockbook_core::{
    create_file_at_path, get_file_by_path, import_files, CoreError, CreateFileAtPathError,
    GetFileByPathError, ImportFileError,
};
use lockbook_models::file_metadata::DecryptedFileMetadata;

use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};

pub fn copy(disk_paths: &[PathBuf], lb_path: &str) -> CliResult<()> {
    account()?;

    let total = Cell::new(0);
    let nth_file = Cell::new(0);
    let update_status = move |status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n_files) => total.set(n_files),
        ImportStatus::Error(disk_path, err) => match err {
            CoreError::DiskPathInvalid => eprintln!("invalid disk path '{}'", disk_path.display()),
            _ => eprintln!("unexpected error: {:#?}", err),
        },
        ImportStatus::StartingItem(disk_path) => {
            nth_file.set(nth_file.get() + 1);
            print!("({}/{}) Importing: {}... ", nth_file.get(), total.get(), disk_path);
            std::io::stdout().flush().unwrap();
        }
        ImportStatus::FinishedItem(_meta) => println!("Done."),
    };

    let dest = get_or_create_file(lb_path)?;
    let dest_name = dest.decrypted_name;

    import_files(&config()?, disk_paths, dest.id, &update_status).map_err(|err| match err {
        lockbook_core::Error::UiError(err) => match err {
            ImportFileError::NoAccount => err!(NoAccount),
            ImportFileError::ParentDoesNotExist => err!(FileNotFound(dest_name)),
            ImportFileError::DocumentTreatedAsFolder => err!(DocTreatedAsFolder(dest_name)),
        },
        lockbook_core::Error::Unexpected(msg) => err_unexpected!("{}", msg),
    })
}

fn get_or_create_file(lb_path: &str) -> CliResult<DecryptedFileMetadata> {
    // Try to get a file
    match get_file_by_path(&config()?, lb_path) {
        Ok(file) => return Ok(file),
        Err(err) => match err {
            lockbook_core::Error::UiError(GetFileByPathError::NoFileAtThatPath) => {} // Continue
            lockbook_core::Error::Unexpected(msg) => return Err(err_unexpected!("{}", msg)),
        },
    };

    // It does not exist, create it
    if lb_path.ends_with('/') {
        create_file_at_path(&config()?, lb_path).map_err(|err| match err {
            lockbook_core::Error::UiError(err) => match err {
                CreateFileAtPathError::FileAlreadyExists => {
                    err!(FileAlreadyExists(lb_path.to_string()))
                }
                CreateFileAtPathError::NoAccount => err!(NoAccount),
                CreateFileAtPathError::NoRoot => err!(NoRoot),
                CreateFileAtPathError::PathContainsEmptyFile => {
                    err!(PathContainsEmptyFile(lb_path.to_string()))
                }
                CreateFileAtPathError::PathDoesntStartWithRoot => {
                    err!(PathNoRoot(lb_path.to_string()))
                }
                CreateFileAtPathError::DocumentTreatedAsFolder => {
                    err!(DocTreatedAsFolder(lb_path.to_string()))
                }
            },
            lockbook_core::Error::Unexpected(msg) => err_unexpected!("{}", msg),
        })
    } else {
        eprintln!("Copy destination must be a folder!");
        Err(err!(DocTreatedAsFolder(lb_path.to_string())))
    }
}
