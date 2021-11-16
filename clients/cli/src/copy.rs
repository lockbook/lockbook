use std::path::PathBuf;

use lockbook_core::{
    create_file_at_path, get_file_by_path, import_file, CreateFileAtPathError, Error as CoreError,
    Error, GetFileByPathError, ImportFileError,
};

use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};
use lockbook_core::service::import_export_service::ImportExportFileInfo;
use lockbook_models::file_metadata::DecryptedFileMetadata;

pub fn copy(disk_paths: &[PathBuf], lb_path: &str) -> CliResult<()> {
    account()?;

    let file_metadata = get_or_create_file(lb_path)?;

    let import_progress = |info: ImportExportFileInfo| {
        println!(
            "importing: {} to {}",
            info.disk_path.display(),
            info.lockbook_path
        );
    };

    for path in disk_paths {
        import_file(
            &config()?,
            path.to_path_buf(),
            file_metadata.id,
            Some(Box::new(import_progress)),
        )
        .map_err(|err| match err {
            CoreError::UiError(err) => match err {
                ImportFileError::NoAccount => err!(NoAccount),
                ImportFileError::ParentDoesNotExist => {
                    err!(FileNotFound(file_metadata.decrypted_name.clone()))
                }
                ImportFileError::DocumentTreatedAsFolder => {
                    err!(DocTreatedAsFolder(file_metadata.decrypted_name.clone()))
                }
                ImportFileError::DiskPathInvalid => err!(OsInvalidPath(lb_path.to_string())),
            },
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        })?;
    }

    Ok(())
}

fn get_or_create_file(lb_path: &str) -> CliResult<DecryptedFileMetadata> {
    // Try to get a file
    match get_file_by_path(&config()?, lb_path) {
        Ok(file) => return Ok(file),
        Err(err) => match err {
            Error::UiError(GetFileByPathError::NoFileAtThatPath) => {} // Continue
            Error::Unexpected(msg) => return Err(err_unexpected!("{}", msg)),
        },
    };

    // It does not exist, create it
    if lb_path.ends_with('/') {
        create_file_at_path(&config()?, lb_path).map_err(|err| match err {
            CoreError::UiError(err) => match err {
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
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        })
    } else {
        eprintln!("Copy destination must be a folder!");
        Err(err!(DocTreatedAsFolder(lb_path.to_string())))
    }
}
