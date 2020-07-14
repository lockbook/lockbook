use std::thread::sleep;
use std::{env, time};

use chrono::Duration;
use chrono_human_duration::ChronoHumanDuration;

use lockbook_core::model::account::Account;
use lockbook_core::model::state::Config;
use lockbook_core::repo::account_repo::{AccountRepo, Error};
use lockbook_core::repo::db_provider::DbProvider;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::clock_service::Clock;
use lockbook_core::{
    Db, DefaultAccountRepo, DefaultClock, DefaultDbProvider, DefaultFileMetadataRepo,
};

use crate::utils::SupportedEditors::{Code, Emacs, Nano, Sublime, Vim};
use std::process::exit;

pub fn get_config() -> Config {
    let path = env::var("LOCKBOOK_CLI_LOCATION")
        .unwrap_or(format!("{}/.lockbook", env::var("HOME")
            .expect("Could not read env var LOCKBOOK_CLI_LOCATION or HOME, don't know where to place your .lockbook folder"))
        );

    Config {
        writeable_path: path,
    }
}

pub fn exit_with(message: &str, status: u8) {
    if status == 0 {
        println!("{}", message);
    } else {
        eprintln!("{}", message);
    }
    exit(status as i32);
}

pub fn connect_to_db() -> Db {
    // Save data in LOCKBOOK_CLI_LOCATION or ~/.lockbook/
    let path = env::var("LOCKBOOK_CLI_LOCATION")
        .unwrap_or(format!("{}/.lockbook", env::var("HOME")
            .expect("Could not read env var LOCKBOOK_CLI_LOCATION or HOME, don't know where to place your .lockbook folder"))
        );

    let config = Config {
        writeable_path: path,
    };

    // Try to connect 3 times waiting 10ms and 150ms
    // If there's a particularly long write or something going on the write would be blocked, we don't
    // want to panic in this case when waiting a very small amount of time would do fine
    match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => db,
        Err(_) => {
            sleep(time::Duration::from_millis(10));
            match DefaultDbProvider::connect_to_db(&config) {
                Ok(db) => db,
                Err(_) => {
                    sleep(time::Duration::from_millis(100));
                    match DefaultDbProvider::connect_to_db(&config) {
                        Ok(db) => db,
                        Err(_) => {
                            sleep(time::Duration::from_millis(2000));
                            match DefaultDbProvider::connect_to_db(&config) {
                                Ok(db) => db,
                                Err(err) => panic!("Could not connect to db! Error: {:?}", err),
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn get_account(db: &Db) -> Account {
    // DefaultAccountRepo::get_account(&db).expect("test")
    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => account,
        Err(err) => match err {
            Error::SledError(err) => {
                panic!("No account found, run init, import or help. Error: {}", err)
            }
            Error::SerdeError(err) => panic!("Account data corrupted: {}", err),
            Error::AccountMissing(err) => panic!(
                "No account found, run init, import or help. Error: {:?}",
                err
            ),
        },
    }
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

pub fn print_last_successful_sync(db: &Db) {
    if atty::is(atty::Stream::Stdout) {
        let last_updated = DefaultFileMetadataRepo::get_last_updated(&db)
            .expect("Failed to retrieve content from FileMetadataRepo");

        let duration = if last_updated != 0 {
            let duration =
                Duration::milliseconds((DefaultClock::get_time() as u64 - last_updated) as i64);
            duration.format_human().to_string()
        } else {
            "never".to_string()
        };

        println!("Last successful sync: {}", duration);
    }
}
