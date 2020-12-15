use std::env;

use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;

use lockbook_core::model::state::Config;
use lockbook_core::service::clock_service::Clock;
use lockbook_core::Error as CoreError;
use lockbook_core::{
    get_account, get_db_state, init_logger, migrate_db, GetAccountError, GetStateError,
    MigrationError,
};
use lockbook_core::{get_last_synced, DefaultClock};

use crate::edit::save_file_to_core;
use crate::utils::SupportedEditors::{Code, Emacs, Nano, Sublime, Vim};
use crate::{
    NETWORK_ISSUE, NO_ACCOUNT, NO_CLI_LOCATION, UNEXPECTED_ERROR, UNINSTALL_REQUIRED,
    UPDATE_REQUIRED,
};
use hotwatch::{Event, Hotwatch};
use lockbook_core::model::account::Account;
use lockbook_core::model::file_metadata::FileMetadata;
use lockbook_core::service::db_state_service::State;
use std::path::Path;
use std::process::exit;

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
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
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
                        CoreError::UiError(MigrationError::StateRequiresCleaning) => exit_with(
                            "Your local state cannot be migrated, please re-sync with a fresh client.",
                            UNINSTALL_REQUIRED,
                        ),
                        CoreError::Unexpected(msg) =>
                            exit_with(
                                &format!(
                                    "An unexpected error occurred while migrating, it's possible you need to clear your local state and resync. Error: {}",
                                    &msg
                                ),
                                UNEXPECTED_ERROR
                            )
                    }
                }
            }
            State::StateRequiresClearing => exit_with(
                "Your local state cannot be migrated, please re-sync with a fresh client.",
                UNINSTALL_REQUIRED,
            ),
        },
        Err(err) => match err {
            CoreError::UiError(GetStateError::Stub) => exit_with("Impossible", UNEXPECTED_ERROR),
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}

pub fn get_config() -> Config {
    let path = match (env::var("LOCKBOOK_CLI_LOCATION"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => s,
        (Err(_), Ok(s), _) => format!("{}/.lockbook", s),
        (Err(_), Err(_), Ok(s)) => format!("{}/.lockbook", s),
        _ => exit_with("Could not read env var LOCKBOOK_CLI_LOCATION HOME or HOMEPATH, don't know where to place your .lockbook folder", NO_CLI_LOCATION)
    };

    Config {
        writeable_path: path,
    }
}

pub fn exit_with_upgrade_required() -> ! {
    exit_with(
        "An update to your application is required to do this action!",
        UPDATE_REQUIRED,
    )
}

pub fn exit_with_offline() -> ! {
    exit_with("Could not reach server!", NETWORK_ISSUE)
}

pub fn exit_with_no_account() -> ! {
    exit_with("No account! Run init or import to get started!", NO_ACCOUNT)
}

pub fn exit_with(message: &str, status: u8) -> ! {
    if status == 0 {
        println!("{}", message);
    } else {
        eprintln!("{}", message);
    }
    exit(status as i32);
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
        let last_updated = get_last_synced(&get_config())
            .expect("Failed to retrieve content from FileMetadataRepo");

        let duration = if last_updated != 0 {
            let duration = Duration::milliseconds(DefaultClock::get_time() - last_updated);
            duration.format_human().to_string()
        } else {
            "never".to_string()
        };

        println!("Last successful sync: {}", duration);
    }
}

pub fn set_up_auto_save(
    watch_file_metadata: FileMetadata,
    watch_file_location: String,
) -> Hotwatch {
    let mut hot_watch = Hotwatch::new_with_custom_delay(core::time::Duration::from_secs(5))
        .unwrap_or_else(|err| {
            exit_with(
                &format!("hotwatch failed to initialize: {:#?}", err),
                UNEXPECTED_ERROR,
            )
        });

    hot_watch
        .watch(watch_file_location.clone(), move |event: Event| {
            if let Event::NoticeWrite(_) = event {
                save_file_to_core(
                    watch_file_metadata.clone(),
                    &watch_file_location,
                    Path::new(watch_file_location.as_str()),
                    true,
                )
            } else if let Event::Write(_) = event {
                save_file_to_core(
                    watch_file_metadata.clone(),
                    &watch_file_location,
                    Path::new(watch_file_location.as_str()),
                    true,
                )
            } else if let Event::Create(_) = event {
                save_file_to_core(
                    watch_file_metadata.clone(),
                    &watch_file_location,
                    Path::new(watch_file_location.as_str()),
                    true,
                )
            }
        })
        .unwrap_or_else(|err| {
            exit_with(
                &format!("hotwatch failed to watch: {:#?}", err),
                UNEXPECTED_ERROR,
            )
        });

    hot_watch
}

pub fn stop_auto_save(mut watcher: Hotwatch, file_location: String) {
    watcher.unwatch(file_location.clone()).unwrap_or_else(|err| {
        exit_with(
            &format!("hotwatch failed to unwatch: {:#?}", err),
            UNEXPECTED_ERROR,
        )
    });
}
