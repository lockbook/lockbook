use std::cell::Cell;
use std::io::Write;
use std::path::PathBuf;

use lockbook_core::Core;
use lockbook_core::CoreError;
use lockbook_core::CreateFileAtPathError;
use lockbook_core::DecryptedFileMetadata;
use lockbook_core::Error as LbError;
use lockbook_core::GetFileByPathError;
use lockbook_core::ImportFileError;
use lockbook_core::ImportStatus;

use crate::error::CliError;

pub fn copy(core: &Core, disk_paths: &[PathBuf], lb_path: &str) -> Result<(), CliError> {
    core.get_account()?;

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

    let dest = get_or_create_file(core, lb_path)?;

    core.import_files(disk_paths, dest.id, &update_status)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                ImportFileError::ParentDoesNotExist => CliError::file_not_found(lb_path),
                ImportFileError::DocumentTreatedAsFolder => CliError::doc_treated_as_dir(lb_path),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
}

fn get_or_create_file(core: &Core, lb_path: &str) -> Result<DecryptedFileMetadata, CliError> {
    // Try to get a file
    match core.get_by_path(lb_path) {
        Ok(file) => return Ok(file),
        Err(err) => match err {
            LbError::UiError(GetFileByPathError::NoFileAtThatPath) => {} // Continue
            LbError::Unexpected(msg) => return Err(CliError::unexpected(msg)),
        },
    };

    // It does not exist, create it
    if lb_path.ends_with('/') {
        core.create_at_path(lb_path).map_err(|err| match err {
            LbError::UiError(err) => match err {
                CreateFileAtPathError::FileAlreadyExists => CliError::file_exists(lb_path),
                CreateFileAtPathError::NoRoot => CliError::no_root(),
                CreateFileAtPathError::PathContainsEmptyFile => {
                    CliError::path_has_empty_file(lb_path)
                }
                CreateFileAtPathError::PathDoesntStartWithRoot => CliError::path_no_root(lb_path),
                CreateFileAtPathError::DocumentTreatedAsFolder => {
                    CliError::doc_treated_as_dir(lb_path)
                }
                CreateFileAtPathError::InsufficientPermission => todo!(), // todo(sharing)
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
    } else {
        eprintln!("Copy destination must be a folder!");
        Err(CliError::doc_treated_as_dir(lb_path))
    }
}
