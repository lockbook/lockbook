use std::path::PathBuf;

use lockbook_core::{
    get_file_by_path, import_file, Error as CoreError, GetFileByPathError, ImportFileError,
};

use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config};
use crate::{err, err_unexpected};
use lockbook_core::service::import_export_service::ImportExportFileInfo;

pub fn copy(disk_path: PathBuf, lb_path: &str, edit: bool) -> CliResult<()> {
    get_account_or_exit();

    let config = get_config();

    let file_metadata = get_file_by_path(&config, lb_path).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(lb_path.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let import_progress = |info: ImportExportFileInfo| {
        println!(
            "{} imported to {}",
            info.disk_path.display(),
            info.lockbook_path
        );
    };

    import_file(
        &config,
        file_metadata.id,
        disk_path,
        edit,
        Some(Box::new(import_progress)),
    )
    .map_err(|err| match err {
        CoreError::UiError(err) => match err {
            ImportFileError::NoAccount => err!(NoAccount),
            ImportFileError::ParentDoesNotExist => err!(FileNotFound(file_metadata.name.clone())),
            ImportFileError::FileAlreadyExists(path) => err!(FileCollision(path)),
            ImportFileError::DocumentTreatedAsFolder => {
                err!(DocTreatedAsFolder(file_metadata.name.clone()))
            }
            ImportFileError::DiskPathInvalid => err!(OsInvalidPath(lb_path.to_string())),
        },
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })
}
