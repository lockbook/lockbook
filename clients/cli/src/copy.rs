use std::fs;
use std::path::PathBuf;

use lockbook_core::lockbook_models::crypto::DecryptedValue;
use lockbook_core::{
    create_file_at_path, get_file_by_path, write_document, CreateFileAtPathError,
    Error as CoreError, GetFileByPathError,
};

use crate::utils::{exit_with, exit_with_no_account, get_account_or_exit, get_config};
use crate::{
    COULD_NOT_GET_OS_ABSOLUTE_PATH, COULD_NOT_READ_OS_FILE, COULD_NOT_READ_OS_METADATA,
    DOCUMENT_TREATED_AS_FOLDER, FILE_ALREADY_EXISTS, NO_ROOT, PATH_CONTAINS_EMPTY_FILE,
    PATH_NO_ROOT, SUCCESS, UNEXPECTED_ERROR, UNIMPLEMENTED,
};

pub fn copy(path: PathBuf, import_dest: &str, edit: bool) {
    get_account_or_exit();

    let metadata = fs::metadata(&path).unwrap_or_else(|err| {
        exit_with(
            &format!("Failed to read file metadata: {}", err),
            COULD_NOT_READ_OS_METADATA,
        )
    });

    if metadata.is_file() {
        let content_to_import = fs::read_to_string(&path).unwrap_or_else(|err| {
            exit_with(
                &format!("Failed to read file: {}", err),
                COULD_NOT_READ_OS_FILE,
            )
        });

        let absolute_path_maybe = fs::canonicalize(&path).unwrap_or_else(|error| {
            exit_with(
                &format!("Failed to get absolute path: {}", error),
                COULD_NOT_GET_OS_ABSOLUTE_PATH,
            )
        });

        let import_dest_with_filename = if import_dest.ends_with('/') {
            match absolute_path_maybe.file_name() {
                Some(name) => match name.to_os_string().into_string() {
                    Ok(string) => format!("{}{}", &import_dest, string),
                    Err(err) => exit_with(
                        format!("Unexpected error while trying to convert an OsString -> Rust String: {:?}", err).as_str(),
                        UNEXPECTED_ERROR,
                    ),
                },
                None => exit_with("Import target does not contain a file name!", UNEXPECTED_ERROR),
            }
        } else {
            import_dest.to_string()
        };

        let file_metadata = match create_file_at_path(&get_config(), &import_dest_with_filename) {
            Ok(file_metadata) => file_metadata,
            Err(err) => match err {
                CoreError::UiError(CreateFileAtPathError::FileAlreadyExists) => {
                    if edit {
                        get_file_by_path(&get_config(), &import_dest_with_filename).unwrap_or_else(|get_err| match get_err {
                            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) |
                            CoreError::Unexpected(_) => exit_with(
                                &format!("Unexpected error: {:?}", get_err),
                                UNEXPECTED_ERROR,
                            ),
                        })
                    } else {
                        exit_with(&format!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", import_dest_with_filename), FILE_ALREADY_EXISTS)
                    }
                }
                CoreError::UiError(CreateFileAtPathError::NoAccount) => exit_with_no_account(),
                CoreError::UiError(CreateFileAtPathError::NoRoot) => exit_with("No root folder, have you synced yet?", NO_ROOT),
                CoreError::UiError(CreateFileAtPathError::DocumentTreatedAsFolder) => exit_with(&format!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest_with_filename), DOCUMENT_TREATED_AS_FOLDER),
                CoreError::UiError(CreateFileAtPathError::PathContainsEmptyFile) => exit_with(&format!("Input destination {} contains an empty file!", import_dest_with_filename), PATH_CONTAINS_EMPTY_FILE),
                CoreError::UiError(CreateFileAtPathError::PathDoesntStartWithRoot) => exit_with("Import destination doesn't start with your root folder.", PATH_NO_ROOT),
                CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            },
        };

        match write_document(
            &get_config(),
            file_metadata.id,
            &DecryptedValue::from(content_to_import),
        ) {
            Ok(_) => exit_with(
                &format!("imported to {}", import_dest_with_filename),
                SUCCESS,
            ),
            Err(err) => exit_with(&format!("Unexpected error: {:#?}", err), UNEXPECTED_ERROR),
        }
    } else {
        exit_with(
            "Copying folders has not been implemented yet!",
            UNIMPLEMENTED,
        )
    }
}
