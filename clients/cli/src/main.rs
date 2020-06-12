use lockbook_core::init_logger_safely;
use structopt::StructOpt;

mod edit;
mod export;
mod import;
mod init;
mod list;
mod new;
mod status;
mod sync;
mod utils;

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

    /// Displays: which files need to be pushed or pulled.
    /// If conflicts need to be resolved. And when the last successful sync was.
    Status,

    /// Delete the Lockbook data directory from this device
    Nuke,

    /// Export your private key
    Export,
}

fn main() {
    init_logger_safely();
    let args: Lockbook = Lockbook::from_args();
    match args {
        Lockbook::New => new::new(),
        Lockbook::Sync => sync::sync(),
        Lockbook::Edit => edit::edit(),
        Lockbook::Browse => unimplemented!(),
        Lockbook::Remove => unimplemented!(),
        Lockbook::Move => unimplemented!(),
        Lockbook::Find => unimplemented!(),
        Lockbook::List => list::list(),
        Lockbook::Copy => unimplemented!(),
        Lockbook::Share => unimplemented!(),
        Lockbook::Init => init::init(),
        Lockbook::Import => import::import(),
        Lockbook::Status => status::status(),
        Lockbook::Nuke => unimplemented!(),
        Lockbook::Export => export::export(),
    }
}
