use std::path::PathBuf;

use lockbook_core::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use structopt::StructOpt;

use crate::utils::{check_and_perform_migrations, init_logger_or_print};

mod backup;
mod calculate_usage;
mod copy;
mod edit;
mod export_private_key;
mod import_private_key;
mod list;
mod move_file;
mod new;
mod new_account;
mod print;
mod remove;
mod rename;
mod status;
mod sync;
mod utils;
mod whoami;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A secure and intuitive notebook.")]
enum Lockbook {
    /// Bring a file from your computer into a target destination inside your Lockbook. If your
    /// Lockbook target destination is a Folder, the file name will be taken from the file-system
    /// file.
    Copy {
        /// Edit the file if it exists already
        #[structopt(long)]
        edit: bool,

        /// File on your local filesystem to be copied into Lockbook
        file: PathBuf,

        /// Destination within your Lockbook to put the file
        destination: String,
    },

    /// Open a document for editing
    Edit { path: String },

    /// Export your private key
    ExportPrivateKey,

    /// Import an existing Lockbook
    ImportPrivateKey,

    /// Create a new Lockbook account
    NewAccount,

    /// List all your paths
    List,

    /// List all your files (the things you can filter for rename and move)
    #[structopt(name = "list-all")]
    ListAll,

    /// List only documents (the things you can filter for edit)
    ListDocs,

    /// List all your folders (the things you can filter for the start of new)
    ListFolders,

    /// Move a specified file such that it has the target parent (list-all for first parameter
    /// list-folders for second parameter)
    Move { target: String, new_parent: String },

    /// Create a new document or folder
    New { path: String },

    /// Print the contents of a file
    Print { path: String },

    /// Rename a file at a path to a target value
    Rename { path: String, name: String },

    /// Move a file to trash TODO
    Remove { path: String },

    /// What operations a sync would perform
    Status,

    /// Get updates, push changes
    Sync,

    /// Display Lockbook username
    #[structopt(name = "whoami")]
    WhoAmI,

    /// Backup your Lockbook files and structure to the current directory
    Backup,

    /// Calculate how much space your Lockbook is occupying
    GetUsage {
        /// Show the amount in bytes, don't show a human readable interpretation
        #[structopt(long)]
        exact: bool,
    },
}

fn main() {
    init_logger_or_print();
    let args: Lockbook = Lockbook::from_args();
    check_and_perform_migrations();

    match args {
        Lockbook::Copy {
            file,
            destination,
            edit,
        } => copy::copy(file, &destination, edit),
        Lockbook::Edit { path } => edit::edit(&path.trim()),
        Lockbook::ExportPrivateKey => export_private_key::export_private_key(),
        Lockbook::ImportPrivateKey => import_private_key::import_private_key(),
        Lockbook::NewAccount => new_account::new_account(),
        Lockbook::List => list::list(Some(LeafNodesOnly)),
        Lockbook::ListAll => list::list(None),
        Lockbook::ListDocs => list::list(Some(DocumentsOnly)),
        Lockbook::ListFolders => list::list(Some(FoldersOnly)),
        Lockbook::Move { target, new_parent } => move_file::move_file(&target, &new_parent),
        Lockbook::New { path } => new::new(&path.trim()),
        Lockbook::Print { path } => print::print(&path.trim()),
        Lockbook::Remove { path } => remove::remove(&path.trim()),
        Lockbook::Rename { path, name } => rename::rename(&path, &name),
        Lockbook::Status => status::status(),
        Lockbook::Sync => sync::sync(),
        Lockbook::WhoAmI => whoami::whoami(),
        Lockbook::Backup => backup::backup(),
        Lockbook::GetUsage { exact } => calculate_usage::calculate_usage(exact),
    }
}

// Exit Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html
static SUCCESS: u8 = 0;

static USERNAME_TAKEN: u8 = 1;
static USERNAME_INVALID: u8 = 3;
static NETWORK_ISSUE: u8 = 4;
static UNEXPECTED_ERROR: u8 = 5;
static EXPECTED_STDIN: u8 = 6;
static ACCOUNT_STRING_CORRUPTED: u8 = 7;
static NO_ACCOUNT: u8 = 8;
static FILE_ALREADY_EXISTS: u8 = 9;
static NO_ROOT: u8 = 10;
static PATH_NO_ROOT: u8 = 11;
static DOCUMENT_TREATED_AS_FOLDER: u8 = 12;
static COULD_NOT_READ_OS_METADATA: u8 = 13;
static UNIMPLEMENTED: u8 = 14;
static COULD_NOT_READ_OS_FILE: u8 = 15;
static COULD_NOT_GET_OS_ABSOLUTE_PATH: u8 = 16;
static FILE_NOT_FOUND: u8 = 17;
static COULD_NOT_WRITE_TO_OS_FILE: u8 = 18;
static COULD_NOT_DELETE_OS_FILE: u8 = 18;
static NAME_CONTAINS_SLASH: u8 = 19;
static FILE_NAME_NOT_AVAILABLE: u8 = 20;
static ACCOUNT_ALREADY_EXISTS: u8 = 21;
static ACCOUNT_DOES_NOT_EXIST: u8 = 22;
static USERNAME_PK_MISMATCH: u8 = 23;
static NO_CLI_LOCATION: u8 = 24;
static UPDATE_REQUIRED: u8 = 25;
static UNINSTALL_REQUIRED: u8 = 26;
static PATH_CONTAINS_EMPTY_FILE: u8 = 27;
static NAME_EMPTY: u8 = 28;
static NO_ROOT_OPS: u8 = 29;
static PWD_MISSING: u8 = 30;
static COULD_NOT_CREATE_OS_DIRECTORY: u8 = 31;
static COULD_NOT_MOVE_FOLDER_INTO_ITSELF: u8 = 32;
static COULD_NOT_DELETE_ROOT: u8 = 33;
