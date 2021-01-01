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
    DOCUMENT_TREATED_AS_FOLDER, FILE_ALREADY_EXISTS, NO_ROOT, PATH_CONTAINS_EMPTY_FILE,
    PATH_NO_ROOT, SUCCESS, UNEXPECTED_ERROR,
};

struct LbCliError {
    code: u8,
    msg: String,
}

impl LbCliError {
    fn new(code: u8, msg: String) -> Self {
        Self { code, msg }
    }
}

pub fn copy(path: PathBuf, import_dest: &str, edit: bool) {
    get_account_or_exit();

    let config = get_config();

    if path.is_file() {
        match copy_file(&path, import_dest, &config, edit) {
            Ok(msg) => exit_with(&msg, SUCCESS),
            Err(err) => exit_with(&err.msg, err.code),
        }
    } else {
        let import_dir = match import_dest.ends_with('/') {
            true => import_dest.to_string(),
            false => format!("{}/", import_dest),
        };
        let parent = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| {
                exit_with(
                    &format!("Failed to read parent name, OS path: {:?}", path),
                    COULD_NOT_READ_OS_CHILDREN,
                )
            });
        let import_path = format!("{}{}", import_dir, parent);

        recursive_copy_folder(&path, &import_path, &config, edit);
    }
}

fn recursive_copy_folder(path: &PathBuf, import_dest: &str, config: &Config, edit: bool) {
    if path.is_file() {
        match copy_file(&path, import_dest, config, edit) {
            Ok(msg) => println!("{}", msg),
            Err(err) => eprintln!("{}", err.msg),
        }
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

                let lb_child_path = format!("{}/{}", import_dest, child_name);

                recursive_copy_folder(&child_path, &lb_child_path, config, edit);
            }
        } else if let Err(err) = create_file_at_path(config, &import_dest) {
            match err {
                CoreError::UiError(err) => match err {
                    CreateFileAtPathError::FileAlreadyExists => {
                        if !edit {
                            eprintln!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", import_dest)
                        }
                    }
                    CreateFileAtPathError::NoAccount => exit_with_no_account(),
                    CreateFileAtPathError::NoRoot => exit_with_no_root(),
                    CreateFileAtPathError::DocumentTreatedAsFolder => eprintln!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest),
                    CreateFileAtPathError::PathContainsEmptyFile => eprintln!("Input destination {} contains an empty file!", import_dest),
                    CreateFileAtPathError::PathDoesntStartWithRoot => exit_with("Import destination doesn't start with your root folder.", PATH_NO_ROOT),
                }
                CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            }
        }
    }
}

fn copy_file(
    path: &PathBuf,
    import_dest: &str,
    config: &Config,
    edit: bool,
) -> Result<String, LbCliError> {
    let content = fs::read_to_string(&path).map_err(|err| {
        LbCliError::new(
            COULD_NOT_READ_OS_FILE,
            format!("Failed to read file from {:?}, OS error: {}", path, err),
        )
    })?;

    let absolute_path = fs::canonicalize(&path).map_err(|err| {
        LbCliError::new(
            COULD_NOT_GET_OS_ABSOLUTE_PATH,
            format!(
                "Failed to get absolute path from {:?}, OS error: {}",
                path, err
            ),
        )
    })?;

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
            CoreError::UiError(err) => match err {
                CreateFileAtPathError::FileAlreadyExists => {
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
                    } else {
                        return Err(LbCliError::new(FILE_ALREADY_EXISTS, "Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!".to_string()));
                    }
                }
                CreateFileAtPathError::NoAccount => exit_with_no_account(),
                CreateFileAtPathError::NoRoot => exit_with_no_root(),
                CreateFileAtPathError::DocumentTreatedAsFolder => {
                    return Err(LbCliError::new(DOCUMENT_TREATED_AS_FOLDER, format!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest)));
                }
                CreateFileAtPathError::PathContainsEmptyFile => {
                    return Err(LbCliError::new(
                        PATH_CONTAINS_EMPTY_FILE,
                        format!(
                            "Input destination {} contains an empty file!",
                            import_dest_with_filename
                        ),
                    ));
                }
                CreateFileAtPathError::PathDoesntStartWithRoot => exit_with_path_no_root(),
            },
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    match write_document(config, file_metadata.id, content.as_bytes()) {
        Ok(_) => Ok(format!("imported to {}", import_dest_with_filename)),
        Err(err) => Err(LbCliError::new(
            UNEXPECTED_ERROR,
            format!("Unexpected error: {:#?}", err),
        )),
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

fn exit_with_no_root() -> ! {
    exit_with("No root folder, have you synced yet?", NO_ROOT)
}

fn exit_with_path_no_root() -> ! {
    exit_with(
        "Import destination doesn't start with your root folder.",
        PATH_NO_ROOT,
    )
}
