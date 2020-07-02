use std::path::PathBuf;

use structopt::StructOpt;

use lockbook_core::init_logger_safely;

mod copy;
mod edit;
mod export;
mod import;
mod init;
mod list;
mod new;
mod print;
mod status;
mod sync;
mod utils;
mod whoami;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A secure and intuitive notebook.")]
enum Lockbook {
    /// Create a new file
    New,

    /// Get updates, push changes
    Sync,

    /// Search and edit a file
    Edit,

    /// Search and delete a file
    Remove,

    /// Rename a file
    Move,

    /// List all your files
    List,

    /// Bring a file from your computer into Lockbook
    Copy { file: PathBuf },

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

    /// Print the contents of a file
    Print,

    /// Display lockbook username
    #[structopt(name = "whoami")]
    WhoAmI,
}

fn main() {
    init_logger_safely();
    let args: Lockbook = Lockbook::from_args();
    match args {
        Lockbook::New => new::new(),
        Lockbook::Sync => sync::sync(),
        Lockbook::Edit => edit::edit(),
        Lockbook::Remove => unimplemented!(),
        Lockbook::Move => unimplemented!(),
        Lockbook::List => list::list(),
        Lockbook::Init => init::init(),
        Lockbook::Import => import::import(),
        Lockbook::Status => status::status(),
        Lockbook::Nuke => unimplemented!(),
        Lockbook::Export => export::export(),
        Lockbook::WhoAmI => whoami::whoami(),
        Lockbook::Print => print::print(),
        Lockbook::Copy { file: path } => copy::copy(path),
    }
}
