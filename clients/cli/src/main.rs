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
    /// Create a new file
    New { path: String },

    /// Get updates, push changes
    Sync,

    /// Search and edit a file
    Edit { path: String },

    /// Search and delete a file
    Remove { path: String },

    /// List all your files
    List,

    /// List only documents (starting set for a filter for edit)
    #[structopt(name = "list-docs")]
    ListDocs,

    /// List all your files (starting set for a filter for rename, or delete)
    #[structopt(name = "list-all")]
    ListAll,

    /// List all your folders (starting set for a filter for new)
    #[structopt(name = "list-folders")]
    ListFolders,

    /// Bring a file from your computer into Lockbook
    Copy { file: PathBuf },

    /// Create a new Lockbook account
    Init,

    /// Import an existing Lockbook
    Import,

    /// What operations a sync would perform
    Status,

    /// Export your private key
    Export,

    /// Print the contents of a file
    Print { path: String },

    /// Display lockbook username
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
