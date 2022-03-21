use lockbook_core::model::state::Config;
use lockbook_core::{
    get_account, get_db_state, get_last_synced_human_string, init_logger, list_metadatas,
    migrate_db, GetAccountError, MigrationError,
};
use lockbook_core::{write_document, Error as CoreError, WriteToDocumentError};
use std::{env, fs};
use std::path::PathBuf;

use crate::error::CliResult;
use crate::utils::SupportedEditors::{Code, Emacs, Notepad, Nano, Sublime, Vim};
use crate::{err, err_extra, err_unexpected};
use hotwatch::{Event, Hotwatch};
use lockbook_core::pure_functions::drawing::SupportedImageFormats;
use lockbook_core::pure_functions::drawing::SupportedImageFormats::*;
use lockbook_core::service::db_state_service::State;
use lockbook_models::account::Account;
use lockbook_models::file_metadata::DecryptedFileMetadata;
use uuid::Uuid;

#[macro_export]
macro_rules! path_string {
    ($pb:expr) => {
        $pb.to_string_lossy().to_string()
    };
}

pub fn init_logger_or_print() -> CliResult<()> {
    Ok(init_logger(config()?.path())?)
}

pub fn account() -> CliResult<Account> {
    match get_account(&config()?) {
        Ok(account) => Ok(account),
        Err(err) => match err {
            CoreError::UiError(GetAccountError::NoAccount) => Err(err!(NoAccount)),
            CoreError::Unexpected(msg) => Err(err_unexpected!("{}", msg)),
        },
    }
}

pub fn check_and_perform_migrations() -> CliResult<()> {
    let state = get_db_state(&config()?).map_err(|err| err_unexpected!("{}", err))?;

    match state {
        State::ReadyToUse => {}
        State::Empty => {}
        State::MigrationRequired => {
            if atty::is(atty::Stream::Stdout) {
                println!("Local state requires migration! Performing migration now...");
            }
            migrate_db(&config()?).map_err(|err| match err {
                CoreError::UiError(MigrationError::StateRequiresCleaning) => {
                    err!(UninstallRequired)
                }
                CoreError::Unexpected(msg) => err_extra!(
                    Unexpected(msg),
                    "It's possible you need to clear your local state and resync."
                ),
            })?;

            if atty::is(atty::Stream::Stdout) {
                println!("Migration Successful!");
            }
        }
        State::StateRequiresClearing => return Err(err!(UninstallRequired)),
    }

    Ok(())
}

pub fn config() -> CliResult<Config> {
    let path = match (env::var("LOCKBOOK_CLI_LOCATION"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => Ok(s),
        (Err(_), Ok(s), _) => Ok(format!("{}/.lockbook", s)),
        (Err(_), Err(_), Ok(s)) => Ok(format!("{}/.lockbook", s)),
        _ => Err(err!(NoCliLocation)),
    };

    Ok(Config { writeable_path: path? })
}

// In ascending order of superiority
#[derive(Debug)]
pub enum SupportedEditors {
    Vim,
    Emacs,
    Nano,
    Sublime,
    Code,
    Notepad,
}

pub fn get_editor() -> SupportedEditors {
    match env::var("LOCKBOOK_EDITOR") {
        Ok(editor) => match editor.to_lowercase().as_str() {
            "vim" => Vim,
            "emacs" => Emacs,
            "nano" => Nano,
            "subl" | "sublime" => Sublime,
            "code" => Code,
            "notepad" => Notepad,
            _ => {
                let default_editor = if cfg!(target_os = "windows") { Notepad } else { Vim };
                eprintln!(
                    "{} is not yet supported, make a github issue! Falling back to {:?}.",
                    editor, default_editor
                );
                default_editor
            }
        },
        Err(_) => {
            let editor = if cfg!(target_os = "windows") { Notepad } else { Vim };
            eprintln!("LOCKBOOK_EDITOR not set, assuming {:?}", editor);
            editor
        }
    }
}

pub fn get_directory_location() -> CliResult<PathBuf> {
    let mut result = env::temp_dir();
    result.push(format!("{}", Uuid::new_v4()));
    fs::create_dir(&result)
        .map_err(|err| err_unexpected!("couldn't open temporary file for writing: {:#?}", err))?;
    Ok(result)
}

pub fn get_image_format(image_format: &str) -> SupportedImageFormats {
    match image_format.to_lowercase().as_str() {
        "png" => Png,
        "jpeg" | "jpg" => Jpeg,
        "bmp" => Bmp,
        "tga" => Tga,
        "pnm" => Pnm,
        "farbfeld" => Farbfeld,
        _ => {
            eprintln!(
                "{} is not yet supported, make a github issue! Falling back to png.",
                image_format
            );
            Png
        }
    }
}

#[cfg(target_os = "windows")]
pub fn edit_file_with_editor(file_location: &PathBuf) -> bool {
    // if cfg!(target_os = "windows") 
        let command = match get_editor() {
            Vim | Emacs | Nano => {
                eprintln!("Terminal editors are not supported on windows! Set LOCKBOOK_EDITOR to a visual editor.");
                return false;
            }
            Sublime => "subl --wait",
            Code => "code --wait",
            Notepad => "call notepad.exe"
        };

        std::process::Command::new("cmd")
            .arg("/C")
            .arg(command)
            .arg(file_location)
            .spawn()
            .expect("Error: Failed to run editor")
            .wait()
            .unwrap()
            .success()
}

#[cfg(not(target_os = "windows"))]
pub fn edit_file_with_editor(file_location: &PathBuf) -> bool {
        let command = match get_editor() {
            Notepad => {
                eprintln!("Notepad is only supported on windows! Set LOCKBOOK_EDITOR to a different editor.");
                return false
            }
            Vim => "</dev/tty vim",
            Emacs => "</dev/tty emacs",
            Nano => "</dev/tty nano",
            Sublime => "subl --wait",
            Code => "code --wait",
        };

        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .arg(file_location)
            .spawn()
            .expect("Error: Failed to run editor")
            .wait()
            .unwrap()
            .success()
    
}

pub fn metadatas() -> CliResult<Vec<DecryptedFileMetadata>> {
    list_metadatas(&config()?).map_err(|err| err_unexpected!("{}", err))
}

pub fn print_last_successful_sync() -> CliResult<()> {
    if atty::is(atty::Stream::Stdout) {
        let last_updated = get_last_synced_human_string(&config()?)
            .map_err(|err| err_unexpected!("attempting to retrieve usage: {:#?}", err))?;

        println!("Last successful sync: {}", last_updated);
    }
    Ok(())
}

pub fn set_up_auto_save(id: Uuid, location: PathBuf) -> Option<Hotwatch> {
    match Hotwatch::new_with_custom_delay(core::time::Duration::from_secs(5)) {
        Ok(mut watcher) => {
            watcher
                .watch(location.clone(), move |event: Event| match event {
                    Event::NoticeWrite(_) | Event::Write(_) | Event::Create(_) => {
                        if let Err(err) = save_temp_file_contents(id, &location) {
                            err.print();
                        }
                    }
                    _ => {}
                })
                .unwrap_or_else(|err| {
                    println!("file watcher failed to watch: {:#?}", err);
                });

            Some(watcher)
        }
        Err(err) => {
            println!("file watcher failed to initialize: {:#?}", err);
            None
        }
    }
}

pub fn stop_auto_save(mut watcher: Hotwatch, file_location: PathBuf) {
    watcher
        .unwatch(file_location)
        .unwrap_or_else(|err| eprintln!("file watcher failed to unwatch: {:#?}", err))
}

pub fn save_temp_file_contents(id: Uuid, location: &PathBuf) -> CliResult<()> {
    let secret = fs::read_to_string(&location)
        .map_err(|err| {
            err_unexpected!(
                "could not read from temporary file, not deleting {:?}, err: {:#?}",
                location,
                err
            )
        })?
        .into_bytes();

    write_document(&config()?, id, &secret).map_err(|err| match err {
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        CoreError::UiError(WriteToDocumentError::NoAccount) => {
            err_unexpected!("No account! Run 'new-account' or 'import-private-key' to get started!")
        }
        CoreError::UiError(WriteToDocumentError::FileDoesNotExist) => {
            err_unexpected!("FileDoesNotExist")
        }
        CoreError::UiError(WriteToDocumentError::FolderTreatedAsDocument) => {
            err_unexpected!("CannotWriteToFolder")
        }
    })
}
