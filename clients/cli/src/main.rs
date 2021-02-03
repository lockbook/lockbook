use std::path::PathBuf;

use lockbook_core::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use structopt::StructOpt;

use crate::utils::{check_and_perform_migrations, init_logger_or_print};

mod backup;
mod calculate_usage;
mod copy;
mod edit;
mod error;
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
    Remove {
        path: String,
        /// Skip the confirmation check for a folder
        #[structopt(short, long)]
        force: bool,
    },

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

    /// Print out what each error code means
    Errors,
}

fn main() {
    init_logger_or_print();
    let args: Lockbook = Lockbook::from_args();
    check_and_perform_migrations();

    if let Err(err) = match args {
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
        Lockbook::Remove { path, force } => remove::remove(&path.trim(), force),
        Lockbook::Rename { path, name } => rename::rename(&path, &name),
        Lockbook::Status => status::status(),
        Lockbook::Sync => sync::sync(),
        Lockbook::WhoAmI => whoami::whoami(),
        Lockbook::Backup => backup::backup(),
        Lockbook::GetUsage { exact } => calculate_usage::calculate_usage(exact),
        Lockbook::Errors => error::ErrorKind::print_table(),
    } {
        err.exit()
    }
}
