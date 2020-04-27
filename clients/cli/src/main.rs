
use structopt::StructOpt;


mod utils;
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
