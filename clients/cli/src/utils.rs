use std::path::{Path, PathBuf};
use std::{env, fs};

use hotwatch::{Event, Hotwatch};

use lockbook_core::{Core, File, Uuid};
use lockbook_core::{Error as LbError, GetFileByPathError, WriteToDocumentError};

use crate::error::CliError;

// In ascending order of superiority
#[derive(Debug)]
pub enum SupportedEditors {
    Vim,
    Emacs,
    Nano,
    Sublime,
    Code,
}

pub fn get_editor() -> SupportedEditors {
    use SupportedEditors::*;
    let default_editor = if cfg!(target_os = "windows") { Code } else { Vim };
    match env::var("LOCKBOOK_EDITOR") {
        Ok(editor) => match editor.to_lowercase().as_str() {
            "vim" => Vim,
            "emacs" => Emacs,
            "nano" => Nano,
            "subl" | "sublime" => Sublime,
            "code" => Code,
            _ => {
                eprintln!(
                    "{} is not yet supported, make a github issue! Falling back to {:?}.",
                    editor, default_editor
                );
                default_editor
            }
        },
        Err(_) => {
            eprintln!("LOCKBOOK_EDITOR not set, assuming {:?}", default_editor);
            default_editor
        }
    }
}

pub fn get_by_path(core: &Core, path: &str) -> Result<File, CliError> {
    core.get_by_path(path).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(path),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })
}

pub fn get_directory_location() -> Result<PathBuf, CliError> {
    let mut dir = env::temp_dir();
    dir.push(Uuid::new_v4().to_string());
    fs::create_dir(&dir).map_err(|err| {
        CliError::unexpected(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;
    Ok(dir)
}

#[cfg(target_os = "windows")]
pub fn edit_file_with_editor<S: AsRef<Path>>(path: S) -> bool {
    use SupportedEditors::*;

    let path_str = path.as_ref().display();

    let command = match get_editor() {
        Vim | Emacs | Nano => {
            eprintln!("Terminal editors are not supported on windows! Set LOCKBOOK_EDITOR to a visual editor.");
            return false;
        }
        Sublime => format!("subl --wait {}", path_str),
        Code => format!("code --wait {}", path_str),
    };

    std::process::Command::new("cmd")
        .arg("/C")
        .arg(command)
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .unwrap()
        .success()
}

#[cfg(not(target_os = "windows"))]
pub fn edit_file_with_editor<S: AsRef<Path>>(path: S) -> bool {
    use SupportedEditors::*;

    let path_str = path.as_ref().display();

    let command = match get_editor() {
        Vim => format!("</dev/tty vim {}", path_str),
        Emacs => format!("</dev/tty emacs {}", path_str),
        Nano => format!("</dev/tty nano {}", path_str),
        Sublime => format!("subl --wait {}", path_str),
        Code => format!("code --wait {}", path_str),
    };

    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .unwrap()
        .success()
}

pub fn print_last_successful_sync(core: &Core) -> Result<(), CliError> {
    if atty::is(atty::Stream::Stdout) {
        let last_updated = core.get_last_synced_human_string().map_err(|err| {
            CliError::unexpected(format!("attempting to retrieve usage: {:#?}", err))
        })?;

        println!("Last successful sync: {}", last_updated);
    }
    Ok(())
}

pub fn set_up_auto_save<P: AsRef<Path>>(core: &Core, id: Uuid, path: P) -> Option<Hotwatch> {
    match Hotwatch::new_with_custom_delay(core::time::Duration::from_secs(5)) {
        Ok(mut watcher) => {
            let core = core.clone();
            let path = PathBuf::from(path.as_ref());

            watcher
                .watch(path.clone(), move |event: Event| match event {
                    Event::NoticeWrite(_) | Event::Write(_) | Event::Create(_) => {
                        if let Err(err) = save_temp_file_contents(&core, id, &path) {
                            err.print();
                        }
                    }
                    _ => {}
                })
                .unwrap_or_else(|err| println!("file watcher failed to watch: {:#?}", err));

            Some(watcher)
        }
        Err(err) => {
            println!("file watcher failed to initialize: {:#?}", err);
            None
        }
    }
}

pub fn stop_auto_save<P: AsRef<Path>>(mut watcher: Hotwatch, path: P) {
    watcher
        .unwatch(path)
        .unwrap_or_else(|err| eprintln!("file watcher failed to unwatch: {:#?}", err))
}

pub fn save_temp_file_contents<P: AsRef<Path>>(
    core: &Core, id: Uuid, path: P,
) -> Result<(), CliError> {
    let secret = fs::read_to_string(&path)
        .map_err(|err| {
            CliError::unexpected(format!(
                "could not read from temporary file, not deleting {}, err: {:#?}",
                path.as_ref().display(),
                err
            ))
        })?
        .into_bytes();

    let path = core.get_path_by_id(id)?;

    core.write_document(id, &secret).map_err(|err| match err {
        LbError::UiError(err) => match err {
            WriteToDocumentError::FileDoesNotExist => CliError::unexpected("file doesn't exist"),
            WriteToDocumentError::FolderTreatedAsDocument => {
                CliError::unexpected("can't write to folder")
            }
            WriteToDocumentError::InsufficientPermission => CliError::no_write_permission(path),
        },
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })
}
