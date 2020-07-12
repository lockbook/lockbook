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
mod new;
mod print;
mod remove;
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

    /// Create a new document or folder
    New { path: String },

    /// Print the contents of a file
    Print { path: String },

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

fn main() {
    init_logger_safely();
    let args: Lockbook = Lockbook::from_args();
    match args {
        Lockbook::New { path } => new::new(&path.trim()),
        Lockbook::Sync => sync::sync(),
        Lockbook::Edit { path } => edit::edit(&path.trim()),
        Lockbook::Remove { path } => remove::remove(&path.trim()),
        Lockbook::List => list::list(Some(LeafNodesOnly)),
        Lockbook::ListAll => list::list(None),
        Lockbook::ListDocs => list::list(Some(DocumentsOnly)),
        Lockbook::ListFolders => list::list(Some(FoldersOnly)),
        Lockbook::Init => init::init(),
        Lockbook::Import => import::import(),
        Lockbook::Status => status::status(),
        Lockbook::Export => export::export(),
        Lockbook::WhoAmI => whoami::whoami(),
        Lockbook::Print { path } => print::print(&path.trim()),
        Lockbook::Copy { file } => copy::copy(file),
    }
}
