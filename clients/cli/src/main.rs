use std::env;
use std::path::PathBuf;

use structopt::StructOpt;

use lockbook_core::Config;
use lockbook_core::Core;
use lockbook_core::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};

use crate::error::CliError;

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
mod tree;
mod utils;
mod validate;
mod whoami;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A secure and intuitive notebook.")]
enum Lockbook {
    /// Backup your Lockbook files and structure to the current directory
    Backup,

    /// Copy a file from your file system into your Lockbook
    Copy {
        /// At-least one filesystem location
        #[structopt(required = true)]
        disk_files: Vec<PathBuf>,

        /// A folder within your Lockbook, will be created if it does not exist
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

    /// Print the file tree with a given file as the head
    Tree,

    /// Display Lockbook username
    #[structopt(name = "whoami")]
    WhoAmI,

    /// Find lockbook file structure problems, corrupted or missing files.
    Validate,
}

fn exit_with(err: CliError) -> ! {
    err.print();
    std::process::exit(err.code as i32)
}

fn parse_and_run() -> Result<(), CliError> {
    let lockbook_dir = match (env::var("LOCKBOOK_PATH"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => s,
        (Err(_), Ok(s), _) => format!("{}/.lockbook", s),
        (Err(_), Err(_), Ok(s)) => format!("{}/.lockbook", s),
        _ => return Err(CliError::no_cli_location()),
    };
    let writeable_path = format!("{}/cli", lockbook_dir);

    let core = Core::init(&Config { logs: true, writeable_path })?;

    match Lockbook::from_args() {
        Lockbook::Copy { disk_files: files, destination } => {
            copy::copy(&core, &files, &destination)
        }
        Lockbook::Edit { path } => edit::edit(&core, path.trim()),
        Lockbook::ExportPrivateKey => export_private_key::export_private_key(&core),
        Lockbook::ImportPrivateKey => import_private_key::import_private_key(&core),
        Lockbook::NewAccount => new_account::new_account(&core),
        Lockbook::List => list::list(&core, Some(LeafNodesOnly)),
        Lockbook::ListAll => list::list(&core, None),
        Lockbook::ListDocs => list::list(&core, Some(DocumentsOnly)),
        Lockbook::ListFolders => list::list(&core, Some(FoldersOnly)),
        Lockbook::Move { target, new_parent } => move_file::move_file(&core, &target, &new_parent),
        Lockbook::New { path } => new::new(&core, path.trim()),
        Lockbook::Print { path } => print::print(&core, path.trim()),
        Lockbook::Remove { path, force } => remove::remove(&core, path.trim(), force),
        Lockbook::Rename { path, name } => rename::rename(&core, &path, &name),
        Lockbook::Status => status::status(&core),
        Lockbook::Sync => sync::sync(&core),
        Lockbook::Tree => tree::tree(&core),
        Lockbook::WhoAmI => whoami::whoami(&core),
        Lockbook::Validate => validate::validate(&core),
        Lockbook::Backup => backup::backup(&core),
        Lockbook::GetUsage { exact } => calculate_usage::calculate_usage(&core, exact),
        Lockbook::ExportDrawing { path, format } => {
            export_drawing::export_drawing(&core, &path, &format)
        }
        Lockbook::Errors => error::print_err_table(),
    }
}

fn main() {
    if let Err(err) = parse_and_run() {
        exit_with(err);
    }
}
