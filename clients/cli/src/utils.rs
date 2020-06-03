use std::env;

use lockbook_core::model::state::Config;

use lockbook_core::repo::db_provider::DbProvider;

use chrono::Duration;
use chrono_human_duration::ChronoHumanDuration;
use lockbook_core::model::account::Account;
use lockbook_core::repo::account_repo::{AccountRepo, Error};
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::clock_service::Clock;
use lockbook_core::{
    Db, DefaultAccountRepo, DefaultClock, DefaultDbProvider, DefaultFileMetadataRepo,
};
use std::process::Command;

pub fn connect_to_db() -> Db {
    // Save data in LOCKBOOK_CLI_LOCATION or ~/.lockbook/
    let path = env::var("LOCKBOOK_CLI_LOCATION")
        .unwrap_or(format!("{}/.lockbook", env::var("HOME")
            .expect("Could not read env var LOCKBOOK_CLI_LOCATION or HOME, don't know where to place your .lockbook folder"))
        );

    DefaultDbProvider::connect_to_db(&Config {
        writeable_path: path.clone(),
    })
    .expect(&format!("Could not connect to db at path: {}", path))
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
            Error::InvalidPrivateKey(err) => {
                panic!("The private key provided is invalid. Error: {:?}", err)
            } // TODO: Ask if it should even panic
        },
    }
}

pub fn get_editor() -> String {
    env::var("VISUAL").unwrap_or_else(|_| env::var("EDITOR").unwrap_or_else(|_| "vi".to_string()))
}

pub fn edit_file_with_editor(file_location: &String) -> bool {
    Command::new(get_editor())
        .arg(&file_location)
        .spawn()
        .expect(
            format!(
                "Failed to spawn: {}, content location: {}",
                get_editor(),
                &file_location
            )
            .as_str(),
        )
        .wait()
        .expect(
            format!(
                "Failed to wait for spawned process: {}, content location: {}",
                get_editor(),
                &file_location
            )
            .as_str(),
        )
        .success()
}

pub fn print_last_successful_sync(db: &Db) {
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
