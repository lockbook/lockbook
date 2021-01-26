use std::{env, fs};

use lockbook_core::model::state::Config;
use lockbook_core::{
    get_account, get_db_state, get_last_synced_human_string, init_logger, migrate_db,
    GetAccountError, GetStateError, MigrationError,
};
use lockbook_core::{write_document, Error as CoreError, WriteToDocumentError};

use crate::error::ErrCode;
use crate::exitlb;
use crate::utils::SupportedEditors::{Code, Emacs, Nano, Sublime, Vim};
use hotwatch::{Event, Hotwatch};
use lockbook_core::model::account::Account;
use lockbook_core::model::file_metadata::FileMetadata;
use lockbook_core::service::db_state_service::State;
use std::path::Path;

pub fn init_logger_or_print() {
    if let Err(err) = init_logger(&get_config().path()) {
        eprintln!("Logger failed to initialize! {:#?}", err)
    }
}

pub fn get_account_or_exit() -> Account {
    match get_account(&get_config()) {
        Ok(account) => account,
        Err(error) => match error {
            CoreError::UiError(GetAccountError::NoAccount) => exit_with_no_account(),
            CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
        },
    }
}

pub fn check_and_perform_migrations() {
    match get_db_state(&get_config()) {
        Ok(state) => match state {
            State::ReadyToUse => {}
            State::Empty => {}
            State::MigrationRequired => {
                if atty::is(atty::Stream::Stdout) {
                    println!("Local state requires migration! Performing migration now...");
                }
                match migrate_db(&get_config()) {
                    Ok(_) => {
                        if atty::is(atty::Stream::Stdout) {
                            println!("Migration Successful!");
                        }
                    }
                    Err(error) => match error {
                        CoreError::UiError(MigrationError::StateRequiresCleaning) => exitlb!(
                            UninstallRequired,
                            "Your local state cannot be migrated, please re-sync with a fresh client."
                        ),
                        CoreError::Unexpected(msg) => exitlb!(
                            Unexpected,
                            "An unexpected error occurred while migrating, it's possible you need to clear your local state and resync. Error: {}",
                            msg
                        )
                    }
                }
            }
            State::StateRequiresClearing => exitlb!(
                UninstallRequired,
                "Your local state cannot be migrated, please re-sync with a fresh client."
            ),
        },
        Err(err) => match err {
            CoreError::UiError(GetStateError::Stub) => exitlb!(Unexpected, "Impossible"),
            CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
        },
    }
}

pub fn get_config() -> Config {
    let path = match (env::var("LOCKBOOK_CLI_LOCATION"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => s,
        (Err(_), Ok(s), _) => format!("{}/.lockbook", s),
        (Err(_), Err(_), Ok(s)) => format!("{}/.lockbook", s),
        _ => exitlb!(NoCliLocation, "Could not read env var LOCKBOOK_CLI_LOCATION HOME or HOMEPATH, don't know where to place your .lockbook folder")
    };

    Config {
        writeable_path: path,
    }
}

pub fn exit_with_upgrade_required() -> ! {
    exitlb!(
        UpdateRequired,
        "An update to your application is required to do this action!"
    )
}

pub fn exit_with_offline() -> ! {
    exitlb!(NetworkIssue, "Could not reach server!")
}

pub fn exit_with_no_account() -> ! {
    exitlb!(NoAccount, "No account! Run init or import to get started!")
}

pub fn exit_success(msgopt: Option<&str>) -> ! {
    if let Some(msg) = msgopt {
        println!("{}", msg);
    }
    std::process::exit(ErrCode::Success as i32)
}

// In order of superiority
pub enum SupportedEditors {
    Vim,
    Emacs,
    Nano,
    Sublime,
    Code,
}

pub fn get_editor() -> SupportedEditors {
    match env::var("LOCKBOOK_EDITOR") {
        Ok(editor) => match editor.to_lowercase().as_str() {
            "vim" => Vim,
            "emacs" => Emacs,
            "nano" => Nano,
            "subl" | "sublime" => Sublime,
            "code" => Code,
            _ => {
                eprintln!(
                    "{} is not yet supported, make a github issue! Falling back to vim",
                    editor
                );
                Vim
            }
        },
        Err(_) => {
            eprintln!("LOCKBOOK_EDITOR not set, assuming vim");
            Vim
        }
    }
}

pub fn edit_file_with_editor(file_location: &str) -> bool {
    let command = match get_editor() {
        Vim => format!("</dev/tty vim {}", file_location),
        Emacs => format!("</dev/tty emacs {}", file_location),
        Nano => format!("</dev/tty nano {}", file_location),
        Sublime => format!("subl --wait {}", file_location),
        Code => format!("code --wait {}", file_location),
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

pub fn print_last_successful_sync() {
    if atty::is(atty::Stream::Stdout) {
        let last_updated = match get_last_synced_human_string(&get_config()) {
            Ok(ok) => ok,
            Err(err) => exitlb!(
                Unexpected,
                "Unexpected error while attempting to retrieve usage: {:#?}",
                err
            ),
        };

        println!("Last successful sync: {}", last_updated);
    }
}

pub fn set_up_auto_save(
    watch_file_metadata: FileMetadata,
    watch_file_location: String,
) -> Option<Hotwatch> {
    let watcher = Hotwatch::new_with_custom_delay(core::time::Duration::from_secs(5));

    match watcher {
        Ok(mut ok) => {
            ok.watch(watch_file_location.clone(), move |event: Event| {
                if let Event::NoticeWrite(_) = event {
                    save_temp_file_contents(
                        watch_file_metadata.clone(),
                        &watch_file_location,
                        Path::new(watch_file_location.as_str()),
                        true,
                    )
                } else if let Event::Write(_) = event {
                    save_temp_file_contents(
                        watch_file_metadata.clone(),
                        &watch_file_location,
                        Path::new(watch_file_location.as_str()),
                        true,
                    )
                } else if let Event::Create(_) = event {
                    save_temp_file_contents(
                        watch_file_metadata.clone(),
                        &watch_file_location,
                        Path::new(watch_file_location.as_str()),
                        true,
                    )
                }
            })
            .unwrap_or_else(|err| {
                println!("file watcher failed to watch: {:#?}", err);
            });

            Some(ok)
        }
        Err(err) => {
            println!("file watcher failed to initialize: {:#?}", err);
            None
        }
    }
}

pub fn stop_auto_save(mut watcher: Hotwatch, file_location: String) {
    watcher
        .unwatch(file_location)
        .unwrap_or_else(|err| exitlb!(Unexpected, "file watcher failed to unwatch: {:#?}", err));
}

pub fn save_temp_file_contents(
    file_metadata: FileMetadata,
    file_location: &String,
    temp_file_path: &Path,
    silent: bool,
) {
    let secret = match fs::read_to_string(temp_file_path) {
        Ok(content) => content.into_bytes(),
        Err(err) => {
            if !silent {
                exitlb!(
                    Unexpected,
                    "Could not read from temporary file, not deleting {}, err: {:#?}",
                    file_location,
                    err
                )
            } else {
                return;
            }
        }
    };

    match write_document(&get_config(), file_metadata.id, &secret) {
        Ok(_) => {
            if !silent {
                exit_success(Some(
                    "Document encrypted and saved. Cleaning up temporary file.",
                ))
            }
        }
        Err(err) => {
            if !silent {
                match err {
                    CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
                    CoreError::UiError(WriteToDocumentError::NoAccount) => exitlb!(
                        Unexpected,
                        "Unexpected: No account! Run init or import to get started!"
                    ),
                    CoreError::UiError(WriteToDocumentError::FileDoesNotExist) => {
                        exitlb!(Unexpected, "Unexpected: FileDoesNotExist")
                    }
                    CoreError::UiError(WriteToDocumentError::FolderTreatedAsDocument) => {
                        exitlb!(Unexpected, "Unexpected: CannotWriteToFolder")
                    }
                }
            }
        }
    }
}
