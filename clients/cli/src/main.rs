use std::path::PathBuf;

use structopt::StructOpt;

use lockbook_core::init_logger_safely;
use lockbook_core::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};

mod copy;
mod edit;
mod export;
mod import;
mod init;
mod list;
mod move_file;
mod new;
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
    /// Bring a file from your computer into your Lockbook
    Copy { file: PathBuf },

    /// Open a document for editing
    Edit { path: String },

    /// Export your private key
    Export,

    /// Import an existing Lockbook
    Import,

    /// Create a new Lockbook account
    Init,

    /// List all your paths
    List,

    /// List all your files (the things you can filter for rename and move)
    #[structopt(name = "list-all")]
    ListAll,

    /// List only documents (the things you can filter for edit)
    #[structopt(name = "list-docs")]
    ListDocs,

    /// List all your folders (the things you can filter for the start of new)
    #[structopt(name = "list-folders")]
    ListFolders,

    /// Move a specified file such that it has the target parent (list-all for first parameter list-folders for second parameter)
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
}

// TODO these should do something this: https://stackoverflow.com/questions/30281235/how-to-cleanly-end-the-program-with-an-exit-code
fn main() {
    init_logger_safely();
    let args: Lockbook = Lockbook::from_args();
    match args {
        Lockbook::Copy { file } => copy::copy(file),
        Lockbook::Edit { path } => edit::edit(&path.trim()),
        Lockbook::Export => export::export(),
        Lockbook::Import => import::import(),
        Lockbook::Init => init::init(),
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
    }
}

// Exit Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html
static SUCCESS: u8 = 0;

// common
static NETWORK_ISSUE: u8 = 4;
static UNEXPECTED_ERROR: u8 = 5;

// init
static USERNAME_TAKEN: u8 = 1;
static USERNAME_INVALID: u8 = 3;
