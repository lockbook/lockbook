use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;

use lockbook_core::model::state::Config;
use lockbook_core::{
    create_file_at_path, get_file_by_path, write_document, CreateFileAtPathError,
    Error as CoreError, GetFileByPathError,
};

use crate::error::Error;
use crate::utils::{exit_success, get_account_or_exit, get_config};
use crate::{err, err_extra, err_unexpected, exitlb, path_string};

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
            .unwrap_or_else(|| exitlb!(OsCouldNotGetFileName(path_string!(path))));
        let import_path = format!("{}{}", import_dir, parent);

        recursive_copy_folder(&path, &import_path, &config, edit);
    }
}

fn recursive_copy_folder(path: &PathBuf, import_dest: &str, config: &Config, edit: bool) {
    if path.is_file() {
        match copy_file(&path, import_dest, config, edit) {
            Ok(msg) => println!("{}", msg),
            Err(err) => err.print(),
        }
    } else {
        let children: Vec<DirEntry> = read_dir_entries_or_exit(&path);

        if !children.is_empty() {
            for child in children {
                let child_path = child.path();
                let child_name = child_path
                    .file_name()
                    .and_then(|child_name| child_name.to_str())
                    .unwrap_or_else(|| exitlb!(OsCouldNotGetFileName(path_string!(child_path))));

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
                    CreateFileAtPathError::NoAccount => exitlb!(NoAccount),
                    CreateFileAtPathError::NoRoot => exitlb!(NoRoot),
                    CreateFileAtPathError::DocumentTreatedAsFolder => eprintln!("A file along the target destination is a document that cannot be used as a folder: {}", import_dest),
                    CreateFileAtPathError::PathContainsEmptyFile => eprintln!("Input destination {} contains an empty file!", import_dest),
                    CreateFileAtPathError::PathDoesntStartWithRoot => exit_with_path_no_root(),
                }
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            }
        }
    }
}

fn copy_file(
    path: &PathBuf,
    import_dest: &str,
    config: &Config,
    edit: bool,
) -> Result<String, Error> {
    let content = fs::read_to_string(&path)
        .map_err(|err| err!(OsCouldNotReadFile(path_string!(path), err)))?;

    let absolute_path = fs::canonicalize(&path)
        .map_err(|err| err!(OsCouldNotGetAbsPath(path_string!(path), err)))?;

    let import_dest_with_filename = if import_dest.ends_with('/') {
        match absolute_path.file_name() {
            Some(name) => match name.to_os_string().into_string() {
                Ok(string) => format!("{}{}", &import_dest, string),
                Err(err) => err_unexpected!("converting an OsString to String: {:?}", err).exit(),
            },
            None => err_unexpected!("Import target does not contain a file name!").exit(),
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
                                    err_unexpected!("{:?}", get_err).exit()
                                }
                            },
                        )
                    } else {
                        return Err(err_extra!(FileAlreadyExists(import_dest_with_filename), "The input destination is not available within lockbook. Use --edit to overwrite the contents of this file!"));
                    }
                }
                CreateFileAtPathError::NoAccount => exitlb!(NoAccount),
                CreateFileAtPathError::NoRoot => exitlb!(NoRoot),
                CreateFileAtPathError::DocumentTreatedAsFolder => {
                    return Err(err!(DocTreatedAsFolder(import_dest_with_filename)));
                }
                CreateFileAtPathError::PathContainsEmptyFile => {
                    return Err(err_extra!(
                        PathContainsEmptyFile(import_dest_with_filename),
                        "The input destination path contains an empty file!"
                    ));
                }
                CreateFileAtPathError::PathDoesntStartWithRoot => exit_with_path_no_root(),
            },
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    };

    match write_document(config, file_metadata.id, content.as_bytes()) {
        Ok(_) => Ok(format!("imported to {}", import_dest_with_filename)),
        Err(err) => Err(err_unexpected!("{:#?}", err)),
    }
}

fn read_dir_entries_or_exit(p: &PathBuf) -> Vec<DirEntry> {
    fs::read_dir(p)
        .unwrap_or_else(|err| exitlb!(OsCouldNotListChildren(path_string!(p), err)))
        .map(|child| child.unwrap_or_else(|err| exitlb!(OsCouldNotReadFile("".to_string(), err))))
        .collect()
}

fn exit_with_path_no_root() -> ! {
    exitlb!(
        PathNoRoot,
        "Import destination doesn't start with your root folder."
    )
}
