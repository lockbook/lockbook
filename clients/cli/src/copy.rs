use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;

use lockbook_core::model::state::Config;
use lockbook_core::{
    create_file_at_path, get_file_by_path, write_document, CreateFileAtPathError,
    Error as CoreError, GetFileByPathError,
};

use crate::error::{CliResult, Error};
use crate::utils::{exit_success, get_account_or_exit, get_config};
use crate::{err, err_extra, err_unexpected, path_string};

pub fn copy(filesystem_path: PathBuf, lockbook_path: &str, edit: bool) -> CliResult<()> {
    get_account_or_exit();

    let config = get_config();

    if filesystem_path.is_file() {
        match copy_file(&filesystem_path, lockbook_path, &config, edit) {
            Ok(msg) => exit_success(&msg),
            Err(err) => Err(err),
        }
    } else {
        let import_dir = match lockbook_path.ends_with('/') {
            true => lockbook_path.to_string(),
            false => format!("{}/", lockbook_path),
        };
        let parent = filesystem_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| err!(OsCouldNotGetFileName(path_string!(filesystem_path))).exit());
        let import_path = format!("{}{}", import_dir, parent);

        recursive_copy_folder(&filesystem_path, &import_path, &config, edit);
        Ok(())
    }
}

fn recursive_copy_folder(
    filesystem_path: &PathBuf,
    lockbook_path: &str,
    config: &Config,
    edit: bool,
) {
    if filesystem_path.is_file() {
        match copy_file(&filesystem_path, lockbook_path, config, edit) {
            Ok(msg) => println!("{}", msg),
            Err(err) => err.print(),
        }
    } else {
        let children: Vec<DirEntry> = read_dir_entries_or_exit(&filesystem_path);

        if !children.is_empty() {
            for child in children {
                let child_path = child.path();
                let child_name = child_path
                    .file_name()
                    .and_then(|child_name| child_name.to_str())
                    .unwrap_or_else(|| {
                        err!(OsCouldNotGetFileName(path_string!(child_path))).exit()
                    });

                let lb_child_path = format!("{}/{}", lockbook_path, child_name);

                recursive_copy_folder(&child_path, &lb_child_path, config, edit);
            }
        } else if let Err(err) = create_file_at_path(config, &lockbook_path) {
            match err {
                CoreError::UiError(err) => match err {
                    CreateFileAtPathError::FileAlreadyExists => {
                        if !edit {
                            eprintln!("Input destination {} not available within lockbook, use --edit to overwrite the contents of this file!", lockbook_path)
                        }
                    }
                    CreateFileAtPathError::NoAccount => err!(NoAccount).exit(),
                    CreateFileAtPathError::NoRoot => err!(NoRoot).exit(),
                    CreateFileAtPathError::DocumentTreatedAsFolder => eprintln!("A file along the target destination is a document that cannot be used as a folder: {}", lockbook_path),
                    CreateFileAtPathError::PathContainsEmptyFile => eprintln!("Input destination {} contains an empty file!", lockbook_path),
                    CreateFileAtPathError::PathDoesntStartWithRoot => err!(PathNoRoot(lockbook_path.to_string())).exit(),
                }
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            }
        }
    }
}

fn copy_file(
    filesystem_path: &PathBuf,
    lockbook_path: &str,
    config: &Config,
    edit: bool,
) -> Result<String, Error> {
    let content = fs::read_to_string(&filesystem_path)
        .map_err(|err| err!(OsCouldNotReadFile(path_string!(filesystem_path), err)))?;

    let absolute_path = fs::canonicalize(&filesystem_path)
        .map_err(|err| err!(OsCouldNotGetAbsPath(path_string!(filesystem_path), err)))?;

    let import_dest_with_filename = if lockbook_path.ends_with('/') {
        match absolute_path.file_name() {
            Some(name) => match name.to_os_string().into_string() {
                Ok(string) => format!("{}{}", &lockbook_path, string),
                Err(err) => err_unexpected!("converting an OsString to String: {:?}", err).exit(),
            },
            None => err_unexpected!("Import target does not contain a file name!").exit(),
        }
    } else {
        lockbook_path.to_string()
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
                CreateFileAtPathError::NoAccount => err!(NoAccount).exit(),
                CreateFileAtPathError::NoRoot => err!(NoRoot).exit(),
                CreateFileAtPathError::DocumentTreatedAsFolder => {
                    return Err(err!(DocTreatedAsFolder(import_dest_with_filename)));
                }
                CreateFileAtPathError::PathContainsEmptyFile => {
                    return Err(err_extra!(
                        PathContainsEmptyFile(import_dest_with_filename),
                        "The input destination path contains an empty file!"
                    ));
                }
                CreateFileAtPathError::PathDoesntStartWithRoot => {
                    err!(PathNoRoot(import_dest_with_filename.to_string())).exit()
                }
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
        .unwrap_or_else(|err| err!(OsCouldNotListChildren(path_string!(p), err)).exit())
        .map(|child| {
            child.unwrap_or_else(|err| err!(OsCouldNotReadFile("".to_string(), err)).exit())
        })
        .collect()
}
