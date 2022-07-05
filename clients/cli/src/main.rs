use std::env;
use std::path::PathBuf;

use structopt::StructOpt;

use lockbook_core::Core;
use lockbook_core::{Config, Uuid};

use crate::error::CliError;

mod backup;
mod calculate_usage;
mod copy;
mod edit;
mod error;
mod export_drawing;
mod list;
mod move_file;
mod new;
mod new_account;
mod print;
mod private_key;
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
        path: Option<String>,

        #[structopt(short, long)]
        id: Option<Uuid>,
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
    PrivateKey {
        #[structopt(short, long)]
        import: bool,

        #[structopt(short, long)]
        export: bool,
    },

    /// How much space does your Lockbook occupy on the server
    GetUsage {
        /// Show the amount in bytes, don't show a human readable interpretation
        #[structopt(long)]
        exact: bool,
    },

    /// List the absolute path of all Lockbook leaf nodes
    List {
        #[structopt(short, long)]
        all: bool,

        #[structopt(short, long)]
        folders: bool,

        #[structopt(short, long)]
        documents: bool,

        #[structopt(short, long)]
        ids: bool,
    },

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
        path: Option<String>,

        #[structopt(short, long)]
        parent: Option<Uuid>,

        #[structopt(short, long)]
        name: Option<String>,
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

    use crate::Lockbook::*;
    match Lockbook::from_args() {
        Copy { disk_files: files, destination } => copy::copy(&core, &files, &destination),
        Edit { path, id } => edit::edit(&core, path, id),
        PrivateKey { import, export } => private_key::private_key(&core, import, export),
        NewAccount => new_account::new_account(&core),
        List { ids, folders, documents, all } => list::list(&core, ids, documents, folders, all),
        Move { target, new_parent } => move_file::move_file(&core, &target, &new_parent),
        New { path, parent, name } => new::new(&core, path, parent, name),
        Print { path } => print::print(&core, path.trim()),
        Remove { path, force } => remove::remove(&core, path.trim(), force),
        Rename { path, name } => rename::rename(&core, &path, &name),
        Status => status::status(&core),
        Sync => sync::sync(&core),
        Tree => tree::tree(&core),
        WhoAmI => whoami::whoami(&core),
        Validate => validate::validate(&core),
        Backup => backup::backup(&core),
        GetUsage { exact } => calculate_usage::calculate_usage(&core, exact),
        ExportDrawing { path, format } => export_drawing::export_drawing(&core, &path, &format),
        Errors => error::print_err_table(),
    }
}

fn main() {
    if let Err(err) = parse_and_run() {
        exit_with(err);
    }
}
