use structopt::StructOpt;
use std::{io, env};
use std::io::Write;
use lockbook_core::{DefaultDbProvider, DefaultAcountService, Db};
use lockbook_core::repo::db_provider::DbProvider;
use lockbook_core::model::state::Config;
use lockbook_core::service::account_service::AccountService;
use lockbook_core::service::account_service::Error;
use lockbook_core::client::NewAccountError;

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
}

fn main() {
    let args: Lockbook = Lockbook::from_args();
    match args {
        Lockbook::New => unimplemented!(),
        Lockbook::Sync => unimplemented!(),
        Lockbook::Edit => unimplemented!(),
        Lockbook::Browse => unimplemented!(),
        Lockbook::Remove => unimplemented!(),
        Lockbook::Move => unimplemented!(),
        Lockbook::Find => unimplemented!(),
        Lockbook::List => unimplemented!(),
        Lockbook::Copy => unimplemented!(),
        Lockbook::Share => unimplemented!(),
        Lockbook::Init => init(),
        Lockbook::Import => unimplemented!(),
        Lockbook::Status => unimplemented!(),
    }
}

fn connect_to_db() -> Db {
    // Save data in LOCKBOOK_CLI_LOCATION or ~/.lockbook/
    let path = env::var("LOCKBOOK_CLI_LOCATION")
        .unwrap_or(format!("{}/.lockbook", env::var("HOME")
            .expect("Could not read env var LOCKBOOK_CLI_LOCATION or HOME, don't know where to place your .lockbook folder"))
        );

    DefaultDbProvider::connect_to_db(&Config { writeable_path: path.clone() })
        .expect(&format!("Could not connect to db at path: {}", path))
}

fn init() {
    let db = connect_to_db();

    print!("Enter a Username: ");
    io::stdout().flush().unwrap();

    let mut username = String::new();
    io::stdin().read_line(&mut username)
        .expect("Failed to read from stdin");
    username.retain(|c| !c.is_whitespace());

    match DefaultAcountService::create_account(&db, username.clone()) {
        Ok(_) => println!("Account created successfully!"),
        Err(err) => match err {
            Error::KeyGenerationError(e) =>
                eprintln!("Could not generate keypair, error: {}", e),

            Error::PersistenceError(_) =>
                eprintln!("Could not persist data, error: "),

            Error::ApiError(api_err) =>
                match api_err {
                    NewAccountError::SendFailed(_) =>
                        eprintln!("Network Error Occurred"),
                    NewAccountError::UsernameTaken =>
                        eprintln!("Username {} not available!", &username),
                    _ =>
                        eprintln!("Unknown Error Occurred!"),
                },

            Error::KeySerializationError(_) =>
                eprintln!("Could not serialize key")
        }
    }
}
