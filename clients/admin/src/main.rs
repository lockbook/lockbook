mod account;
mod disappear;
mod error;
mod indexes;
mod info;
mod validate;

use std::env;

use structopt::StructOpt;

use crate::error::Error;
use crate::indexes::CliIndex;
use lockbook_core::{Config, Core, Uuid};

#[derive(Debug, PartialEq, Eq, StructOpt)]
pub enum Admin {
    /// Disappear a user
    ///
    /// Frees up their username
    DisappearAccount {
        username: String,
    },

    /// Disappear a file
    ///
    /// When you delete a file you flip that file's is_deleted flag to false. In a disaster recovery
    /// scenario, you may want to *disappear* a file so that it never existed. This is useful in a
    /// scenario where your server let in an invalid file.
    DisappearFile {
        id: Uuid,
    },

    /// Validates file trees of all users on the server and prints any failures
    ValidateAccount {
        username: String,
    },

    /// Performs server-wide integrity checks
    ValidateServer,

    /// List all users
    ListUsers {
        #[structopt(short, long)]
        premium: bool,

        #[structopt(short, long)]
        google_play_premium: bool,

        #[structopt(short, long)]
        stripe_premium: bool,
    },

    /// Get a user's info. This includes their username, public key, and payment platform.
    AccountInfo {
        #[structopt(short, long)]
        username: Option<String>,

        // A base 64 encoded and compressed public key
        #[structopt(short, long)]
        public_key: Option<String>,
    },

    RebuildIndex(CliIndex),

    /// Prints information about a file as it appears on the server
    FileInfo {
        id: Uuid,
    },
}

type Res<T> = Result<T, Error>;

pub fn main() {
    let writeable_path = match (env::var("LOCKBOOK_PATH"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => s,
        (Err(_), Ok(s), _) => format!("{}/.lockbook/cli", s),
        (Err(_), Err(_), Ok(s)) => format!("{}/.lockbook/cli", s),
        _ => panic!("no lockbook location"),
    };

    let core = Core::init(&Config { writeable_path, logs: true, colored_logs: true }).unwrap();

    let result = match Admin::from_args() {
        Admin::DisappearAccount { username } => disappear::account(&core, username),
        Admin::ListUsers { premium, google_play_premium, stripe_premium } => {
            account::list(&core, premium, google_play_premium, stripe_premium)
        }
        Admin::AccountInfo { username, public_key } => account::info(&core, username, public_key),
        Admin::DisappearFile { id } => disappear::file(&core, id),
        Admin::ValidateAccount { username } => validate::account(&core, username),
        Admin::ValidateServer => validate::server(&core),
        Admin::FileInfo { id } => info::file(&core, id),
        Admin::RebuildIndex(index) => indexes::rebuild(&core, index),
    };

    if result.is_err() {
        panic!("unsuccessful completion")
    }
}
