use std::fs;
use std::path::PathBuf;

use lockbook_core::{
    create_file_at_path, get_file_by_path, write_document, CreateFileAtPathError,
    Error as CoreError, GetFileByPathError,
};

use crate::utils::{exit_with, exit_with_no_account, get_account_or_exit, get_config};
use crate::{
    COULD_NOT_GET_OS_ABSOLUTE_PATH, COULD_NOT_READ_OS_CHILDREN, COULD_NOT_READ_OS_FILE,
    COULD_NOT_READ_OS_METADATA, DOCUMENT_TREATED_AS_FOLDER, FILE_ALREADY_EXISTS, NO_ROOT,
    PATH_CONTAINS_EMPTY_FILE, PATH_NO_ROOT, SUCCESS, UNEXPECTED_ERROR,
};
use std::fs::DirEntry;

pub fn copy(path: PathBuf, import_dest: &str, edit: bool) {
    get_account_or_exit();

    let metadata = fs::metadata(&path).unwrap_or_else(|err| {
        exit_with(
            &format!("Failed to read file metadata: {}", err),
            COULD_NOT_READ_OS_METADATA,
        )
    });

    if metadata.is_file() {
        copy_file(&path, import_dest, edit, false)
    } else {
        recursive_copy_folder(&path, import_dest, true);
        exit_with(&format!("imported folder to: {}", import_dest), SUCCESS)
    }
}

fn recursive_copy_folder(path: &PathBuf, import_dest: &str, is_top_folder: bool) {
    let metadata = fs::metadata(&path).unwrap_or_else(|err| {
        exit_with(
            &format!("Failed to read file metadata: {}", err),
            COULD_NOT_READ_OS_METADATA,
        )
    });

    if metadata.is_file() {
        copy_file(&path, import_dest, false, true);
    } else {
        let children_paths: Vec<DirEntry> = fs::read_dir(path)
            .unwrap_or_else(|err| {
                exit_with(
                    &format!("Failed to read folder children: {}", err),
                    COULD_NOT_READ_OS_CHILDREN,
                )
            })
            .map(|child| {
                child.unwrap_or_else(|err| {
                    exit_with(
                        &format!("Failed to retrieve child path: {}", err),
                        COULD_NOT_READ_OS_CHILDREN,
                    )
                })
            })
            .collect();

        if !children_paths.is_empty() {
            for child in children_paths {
                let child_path = child.path();
                let child_name = child_path
                    .file_name()
                    .and_then(|child_name| child_name.to_str())
                    .unwrap_or_else(|| {
                        exit_with(
                            &format!(
                                "Failed to read child name, OS parent path: {:?}",
                                child_path
                            ),
                            COULD_NOT_READ_OS_CHILDREN,
                        )
                    });

                let ends_with_slash = import_dest.ends_with('/');
                let lockbook_child_path = if is_top_folder {
                    let parent_name = path
                        .file_name()
                        .and_then(|parent_name| parent_name.to_str())
                        .unwrap_or_else(|| {
                            exit_with(
                                &format!("Failed to read parent name, OS path: {:?}", path),
                                COULD_NOT_READ_OS_CHILDREN,
                            )
                        });

                    format!(
                        "{}{}",
                        import_dest,
                        if ends_with_slash {
                            format!("{}/{}", parent_name, child_name)
                        } else {
                            format!("/{}/{}", parent_name, child_name)
                        }
                    )
                } else {
                    format!(
                        "{}{}",
                        import_dest,
                        if ends_with_slash {
                            child_name.to_string()
                        } else {
                            format!("/{}", child_name)
                        }
                    )
                };

                recursive_copy_folder(&child_path, &lockbook_child_path, false);
            }
        } else if let Err(err) = create_file_at_path(&get_config(), &import_dest) {
            match err {
                CoreError::UiError(CreateFileAtPathError::FileAlreadyExists) => {
                    println!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", import_dest)
                }
                CoreError::UiError(CreateFileAtPathError::NoAccount) => exit_with_no_account(),
                CoreError::UiError(CreateFileAtPathError::NoRoot) => exit_with("No root folder, have you synced yet?", NO_ROOT),
                CoreError::UiError(CreateFileAtPathError::DocumentTreatedAsFolder) => println!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest),
                CoreError::UiError(CreateFileAtPathError::PathContainsEmptyFile) => println!("Input destination {} contains an empty file!", import_dest),
                CoreError::UiError(CreateFileAtPathError::PathDoesntStartWithRoot) => exit_with("Import destination doesn't start with your root folder.", PATH_NO_ROOT),
                CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            }
        }
    }
}

fn copy_file(path: &PathBuf, import_dest: &str, edit: bool, is_folder_copy: bool) {
    let content_to_import = fs::read_to_string(&path);
    let absolute_path_maybe = fs::canonicalize(&path);

    match (content_to_import, absolute_path_maybe) {
        (Ok(content), Ok(absolute_path)) => {
            let import_dest_with_filename = if import_dest.ends_with('/') {
                match absolute_path.file_name() {
                    Some(name) => match name.to_os_string().into_string() {
                        Ok(string) => format!("{}{}", &import_dest, string),
                        Err(err) => exit_with(
                            format!(
                                "Unexpected error while trying to convert an OsString -> Rust String: {:?}",
                                err
                            )
                                .as_str(),
                            UNEXPECTED_ERROR,
                        ),
                    },
                    None => exit_with(
                        "Import target does not contain a file name!",
                        UNEXPECTED_ERROR,
                    ),
                }
            } else {
                import_dest.to_string()
            };

            let file_metadata = match create_file_at_path(&get_config(), &import_dest_with_filename)
            {
                Ok(file_metadata) => file_metadata,
                Err(err) => match err {
                    CoreError::UiError(CreateFileAtPathError::FileAlreadyExists) => {
                        if edit && !is_folder_copy {
                            get_file_by_path(&get_config(), &import_dest_with_filename)
                                .unwrap_or_else(|get_err| match get_err {
                                    CoreError::UiError(GetFileByPathError::NoFileAtThatPath)
                                    | CoreError::Unexpected(_) => exit_with(
                                        &format!("Unexpected error: {:?}", get_err),
                                        UNEXPECTED_ERROR,
                                    ),
                                })
                        } else if !is_folder_copy {
                            exit_with(&format!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", import_dest_with_filename), FILE_ALREADY_EXISTS)
                        } else {
                            return println!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", import_dest_with_filename);
                        }
                    }
                    CoreError::UiError(CreateFileAtPathError::NoAccount) => exit_with_no_account(),
                    CoreError::UiError(CreateFileAtPathError::NoRoot) => {
                        exit_with("No root folder, have you synced yet?", NO_ROOT)
                    }
                    CoreError::UiError(CreateFileAtPathError::DocumentTreatedAsFolder) => {
                        if is_folder_copy {
                            return println!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest);
                        } else {
                            exit_with(&format!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest_with_filename), DOCUMENT_TREATED_AS_FOLDER)
                        }
                    }
                    CoreError::UiError(CreateFileAtPathError::PathContainsEmptyFile) => {
                        if is_folder_copy {
                            return println!(
                                "Input destination {} contains an empty file!",
                                import_dest
                            );
                        } else {
                            exit_with(
                                &format!(
                                    "Input destination {} contains an empty file!",
                                    import_dest_with_filename
                                ),
                                PATH_CONTAINS_EMPTY_FILE,
                            )
                        }
                    }
                    CoreError::UiError(CreateFileAtPathError::PathDoesntStartWithRoot) => {
                        exit_with(
                            "Import destination doesn't start with your root folder.",
                            PATH_NO_ROOT,
                        )
                    }
                    CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
                },
            };

            match write_document(&get_config(), file_metadata.id, content.as_bytes()) {
                Ok(_) => {
                    if is_folder_copy {
                        println!("imported to {}", import_dest_with_filename)
                    } else {
                        exit_with(
                            &format!("imported to {}", import_dest_with_filename),
                            SUCCESS,
                        )
                    }
                }
                Err(err) => exit_with(&format!("Unexpected error: {:#?}", err), UNEXPECTED_ERROR),
            }
        }
        (Err(content_err), _) => {
            if is_folder_copy {
                println!(
                    "Failed to read file from {}, OS error: {}",
                    import_dest, content_err
                )
            } else {
                exit_with(
                    &format!("Failed to read file: {}", content_err),
                    COULD_NOT_READ_OS_FILE,
                )
            }
        }
        (_, Err(path_err)) => {
            if is_folder_copy {
                println!(
                    "Failed to get absolute path from {}, OS error: {}",
                    path_err, path_err
                )
            } else {
                exit_with(
                    &format!("Failed to get absolute path: {}", path_err),
                    COULD_NOT_GET_OS_ABSOLUTE_PATH,
                )
            }
        }
    }
}
