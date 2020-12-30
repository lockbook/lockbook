use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;

use lockbook_core::model::state::Config;
use lockbook_core::{
    create_file_at_path, get_file_by_path, write_document, CreateFileAtPathError,
    Error as CoreError, GetFileByPathError,
};

use crate::utils::{exit_with, exit_with_no_account, get_account_or_exit, get_config};
use crate::{
    COULD_NOT_GET_OS_ABSOLUTE_PATH, COULD_NOT_READ_OS_CHILDREN, COULD_NOT_READ_OS_FILE,
    DOCUMENT_TREATED_AS_FOLDER, FILE_ALREADY_EXISTS, NO_ROOT,
    PATH_CONTAINS_EMPTY_FILE, PATH_NO_ROOT, SUCCESS, UNEXPECTED_ERROR,
};

pub fn copy(path: PathBuf, import_dest: &str, edit: bool) {
    get_account_or_exit();

    let config = get_config();

    if path.is_file() {
        copy_file(&path, import_dest, &config, edit, false)
    } else {
        recursive_copy_folder(&path, import_dest, &config, edit, true);
    }
}

fn recursive_copy_folder(
    path: &PathBuf,
    import_dest: &str,
    config: &Config,
    edit: bool,
    is_top_folder: bool,
) {
    if path.is_file() {
        copy_file(&path, import_dest, config, edit, true);
    } else {
        let children: Vec<DirEntry> = read_dir_entries_or_exit(&path);

        if !children.is_empty() {
            for child in children {
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

                let import_dir = match import_dest.ends_with('/') {
                    true => import_dest.to_string(),
                    false => format!("{}/", import_dest),
                };
                let possible_parent_dir = if is_top_folder {
                    let parent = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_else(|| {
                            exit_with(
                                &format!("Failed to read parent name, OS path: {:?}", path),
                                COULD_NOT_READ_OS_CHILDREN,
                            )
                        });
                    format!("{}/", parent)
                } else {
                    "".to_string()
                };

                let lb_child_path = format!("{}{}{}", import_dir, possible_parent_dir, child_name);

                recursive_copy_folder(&child_path, &lb_child_path, config, edit, false);
            }
        } else if let Err(err) = create_file_at_path(config, &import_dest) {
            match err {
                CoreError::UiError(err) => match err {
                    CreateFileAtPathError::FileAlreadyExists => {
                        if !edit {
                            println!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", import_dest)
                        }
                    }
                    CreateFileAtPathError::NoAccount => exit_with_no_account(),
                    CreateFileAtPathError::NoRoot => exit_with("No root folder, have you synced yet?", NO_ROOT),
                    CreateFileAtPathError::DocumentTreatedAsFolder => println!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest),
                    CreateFileAtPathError::PathContainsEmptyFile => println!("Input destination {} contains an empty file!", import_dest),
                    CreateFileAtPathError::PathDoesntStartWithRoot => exit_with("Import destination doesn't start with your root folder.", PATH_NO_ROOT),
                }
                CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            }
        }
    }
}

fn copy_file(path: &PathBuf, import_dest: &str, config: &Config, edit: bool, is_folder_copy: bool) {
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

            let file_metadata = match create_file_at_path(config, &import_dest_with_filename) {
                Ok(file_metadata) => file_metadata,
                Err(err) => match err {
                    CoreError::UiError(CreateFileAtPathError::FileAlreadyExists) => {
                        if edit {
                            get_file_by_path(config, &import_dest_with_filename).unwrap_or_else(
                                |get_err| match get_err {
                                    CoreError::UiError(GetFileByPathError::NoFileAtThatPath)
                                    | CoreError::Unexpected(_) => exit_with(
                                        &format!("Unexpected error: {:?}", get_err),
                                        UNEXPECTED_ERROR,
                                    ),
                                },
                            )
                        } else if is_folder_copy {
                            return println!(
                                "Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!",
                                import_dest_with_filename
                            );
                        } else {
                            exit_with(&format!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", import_dest_with_filename), FILE_ALREADY_EXISTS);
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

            match write_document(config, file_metadata.id, content.as_bytes()) {
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
                    "Failed to read file from {:?}, OS error: {}",
                    path, content_err
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
                    "Failed to get absolute path from {:?}, OS error: {}",
                    path, path_err
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

fn read_dir_entries_or_exit(p: &PathBuf) -> Vec<DirEntry> {
    fs::read_dir(p)
        .unwrap_or_else(|err| {
            exit_with(
                &format!(
                    "Unable to list children of folder: {:?}, OS error: {}",
                    p, err
                ),
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
        .collect()
}
