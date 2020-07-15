use std::thread::sleep;
use std::{env, time};

use chrono::Duration;
use chrono_human_duration::ChronoHumanDuration;

use lockbook_core::model::state::Config;
use lockbook_core::repo::db_provider::DbProvider;
use lockbook_core::service::clock_service::Clock;
use lockbook_core::{get_last_synced, Db, DefaultClock, DefaultDbProvider};

use crate::utils::SupportedEditors::{Code, Emacs, Nano, Sublime, Vim};
use crate::NO_ACCOUNT;
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
            let duration =
                Duration::milliseconds((DefaultClock::get_time() as u64 - last_updated) as i64);
            duration.format_human().to_string()
        } else {
            "never".to_string()
        };

        println!("Last successful sync: {}", duration);
    }
}
