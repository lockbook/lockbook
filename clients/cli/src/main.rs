use std::env;

use structopt::StructOpt;

use lockbook_core::model::state::Config;

use lockbook_core::repo::db_provider::DbProvider;

use lockbook_core::model::account::Account;
use lockbook_core::repo::account_repo::{AccountRepo, Error};
use lockbook_core::{Db, DefaultAccountRepo, DefaultDbProvider};

mod import;
mod init;
mod list;
mod new;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A secure and intuitive notebook.")]
enum Lockbook {
    /// Create a new file
    New,

    /// Get updates, push changes
    Sync,

    /// Search and edit a file
    Edit,

    /// Browse your files interactively
    Browse,

    /// Search and delete a file
    Remove,

    /// Rename a file
    Move,

    /// Search for a file and see file metadata
    Find,

    /// List all your files
    List,

    /// Bring a file from your computer into Lockbook
    Copy,

    /// Share a file with a collaborator
    Share,

    /// Create a new Lockbook account
    Init,

    /// Import an existing Lockbook
    Import,

    /// See Lockbook's current status
    Status,

    /// Delete the Lockbook data directory from this device
    Nuke,
}

fn main() {
    let args: Lockbook = Lockbook::from_args();
    match args {
        Lockbook::New => new::new(),
        Lockbook::Sync => unimplemented!(),
        Lockbook::Edit => unimplemented!(),
        Lockbook::Browse => unimplemented!(),
        Lockbook::Remove => unimplemented!(),
        Lockbook::Move => unimplemented!(),
        Lockbook::Find => unimplemented!(),
        Lockbook::List => list::list(),
        Lockbook::Copy => unimplemented!(),
        Lockbook::Share => unimplemented!(),
        Lockbook::Init => init::init(),
        Lockbook::Import => import::import(),
        Lockbook::Status => unimplemented!(),
        Lockbook::Nuke => unimplemented!(),
    }
}

fn connect_to_db() -> Db {
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

fn get_account(db: &Db) -> Account {
    // DefaultAccountRepo::get_account(&db).expect("test")
    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => account,
        Err(err) => match err {
            Error::SledError(err) => {
                panic!("No account found, run init, import or help. Error: {}", err)
            }
            Error::SerdeError(err) => panic!("Account data corrupted: {}", err),
            Error::AccountMissing(_) => panic!("No account found, run init, import or help."),
        },
    }
}

fn get_editor() -> String {
    env::var("VISUAL").unwrap_or(env::var("EDITOR").unwrap_or("vi".to_string()))
}
