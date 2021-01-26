use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;

use lockbook_core::model::state::Config;
use lockbook_core::{
    create_file_at_path, get_file_by_path, write_document, CreateFileAtPathError,
    Error as CoreError, GetFileByPathError,
};

use crate::error::ErrCode;
use crate::exitlb;
use crate::utils::{exit_success, exit_with_no_account, get_account_or_exit, get_config};

struct LbCliError {
    code: ErrCode,
    msg: String,
}

impl LbCliError {
    fn new(code: ErrCode, msg: String) -> Self {
        Self { code, msg }
    }

    fn exit(&self) -> ! {
        eprintln!("{}", self.msg);
        std::process::exit(self.code as i32)
    }
}

pub fn copy(path: PathBuf, import_dest: &str, edit: bool) {
    get_account_or_exit();

    let config = get_config();

    if path.is_file() {
        match copy_file(&path, import_dest, &config, edit) {
            Ok(msg) => exit_success(&msg),
            Err(err) => err.exit(),
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
                exitlb!(
                    OsCouldNotReadChildren,
                    "Failed to read parent name, OS path: {:?}",
                    path
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
                        exitlb!(
                            OsCouldNotReadChildren,
                            "Failed to read child name, OS parent path: {:?}",
                            child_path
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
                    CreateFileAtPathError::PathDoesntStartWithRoot => exit_with_path_no_root(),
                }
                CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
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
            ErrCode::OsCouldNotReadFile,
            format!("Failed to read file from {:?}, OS error: {}", path, err),
        )
    })?;

    let absolute_path = fs::canonicalize(&path).map_err(|err| {
        LbCliError::new(
            ErrCode::OsCouldNotGetAbsPath,
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
                Err(err) => exitlb!(
                    Unexpected,
                    "Unexpected error while trying to convert an OsString -> Rust String: {:?}",
                    err
                ),
            },
            None => exitlb!(Unexpected, "Import target does not contain a file name!"),
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
                                | CoreError::Unexpected(_) => {
                                    exitlb!(Unexpected, "Unexpected error: {:?}", get_err)
                                }
                            },
                        )
                    } else {
                        return Err(LbCliError::new(ErrCode::FileAlreadyExists, "Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!".to_string()));
                    }
                }
                CreateFileAtPathError::NoAccount => exit_with_no_account(),
                CreateFileAtPathError::NoRoot => exit_with_no_root(),
                CreateFileAtPathError::DocumentTreatedAsFolder => {
                    return Err(LbCliError::new(ErrCode::DocTreatedAsFolder, format!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest)));
                }
                CreateFileAtPathError::PathContainsEmptyFile => {
                    return Err(LbCliError::new(
                        ErrCode::PathContainsEmptyFile,
                        format!(
                            "Input destination {} contains an empty file!",
                            import_dest_with_filename
                        ),
                    ));
                }
                CreateFileAtPathError::PathDoesntStartWithRoot => exit_with_path_no_root(),
            },
            CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
        },
    };

    match write_document(config, file_metadata.id, content.as_bytes()) {
        Ok(_) => Ok(format!("imported to {}", import_dest_with_filename)),
        Err(err) => Err(LbCliError::new(
            ErrCode::Unexpected,
            format!("Unexpected error: {:#?}", err),
        )),
    }
}

fn read_dir_entries_or_exit(p: &PathBuf) -> Vec<DirEntry> {
    fs::read_dir(p)
        .unwrap_or_else(|err| {
            exitlb!(
                OsCouldNotReadChildren,
                "Unable to list children of folder: {:?}, OS error: {}",
                p,
                err
            )
        })
        .map(|child| {
            child.unwrap_or_else(|err| {
                exitlb!(
                    OsCouldNotReadChildren,
                    "Failed to retrieve child path: {}",
                    err
                )
            })
        })
        .collect()
}

fn exit_with_no_root() -> ! {
    exitlb!(NoRoot, "No root folder, have you synced yet?")
}

fn exit_with_path_no_root() -> ! {
    exitlb!(
        PathNoRoot,
        "Import destination doesn't start with your root folder."
    )
}
