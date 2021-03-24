use std::path::PathBuf;

use structopt::StructOpt;

use lockbook_core::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};

use crate::utils::{check_and_perform_migrations, init_logger_or_print};

mod backup;
mod calculate_usage;
mod copy;
mod edit;
mod error;
mod export_drawing;
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
    /// Backup your Lockbook files and structure to the current directory
    Backup,

    /// Copy a file from your file system into your Lockbook
    Copy {
        /// Overwrite the file if it exists already
        #[structopt(long)]
        edit: bool,

        /// Filesystem location, a folder or individual file
        file: PathBuf,

        /// Lockbook location, for files this can be the parent, or the intended file name within
        /// the parent. For folders, this specifies the parent.
        destination: String,
    },

    /// Open a document for editing
    Edit {
        /// The lockbook location of the file you want to edit. Will use the LOCKBOOK_EDITOR env var
        /// to select an editor. In the absence of this variable, it will default to vim. Editor
        /// options are: Vim, Emacs, Nano, Sublime, Code
        path: String,
    },

    /// Print out what each error code means
    Errors,

    /// Export a drawing as an image
    ExportDrawing {
        /// Path of the drawing within lockbook
        path: String,

        /// Format for export format, options are: png, jpeg, bmp, tga, pnm, farbfeld
        format: String,
    },

    /// Export your private key, if piped, account string, otherwise qr code
    ExportPrivateKey,

    /// How much space does your Lockbook occupy on the server
    GetUsage {
        /// Show the amount in bytes, don't show a human readable interpretation
        #[structopt(long)]
        exact: bool,
    },

    /// Import an account string via stdin
    ImportPrivateKey,

    /// List the absolute path of all Lockbook leaf nodes
    List,

    /// List the absolute path of all lockbook files and folders
    #[structopt(name = "list-all")]
    ListAll,

    /// List the absolute path of your documents
    ListDocs,

    /// List the absolute path of your folders
    ListFolders,

    /// Change the parent of a file or folder
    Move {
        /// File you are moving (lockbook list-all)
        target: String,

        /// New location (lockbook list-folders)
        new_parent: String,
    },

    /// Create a new document or folder
    New {
        /// Absolute path of the file you're creating. Will create folders that do not exist.
        path: String,
    },

    /// Create a new Lockbook account
    NewAccount,

    /// Print the contents of a file to stdout
    Print {
        /// Absolute path of a document (lockbook list-docs)
        path: String,
    },

    /// Delete a file
    Remove {
        /// Absolute path of a file (lockbook list-all)
        path: String,

        /// Skip the confirmation check for a folder
        #[structopt(short, long)]
        force: bool,
    },

    /// Rename a file at a path to a target value
    Rename {
        /// Absolute path of a file (lockbook list-all)
        path: String,

        /// New name
        name: String,
    },

    /// What operations a sync would perform
    Status,

    /// Get updates, push changes
    Sync,

    /// Display Lockbook username
    #[structopt(name = "whoami")]
    WhoAmI,
}

fn main() {
    init_logger_or_print();
    let args = Lockbook::from_args();

    if let Err(err) = check_and_perform_migrations() {
        err.exit()
    }

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
        Lockbook::ExportDrawing { path, format } => export_drawing::export_drawing(&path, &format),
        Lockbook::Errors => error::ErrorKind::print_table(),
    } {
        err.exit()
    }
}
