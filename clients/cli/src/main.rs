use std::env;
use std::path::PathBuf;

use structopt::StructOpt;

use lockbook_core::Core;
use lockbook_core::{Config, Uuid};

use crate::error::CliError;

mod backup;
mod billing;
mod copy;
mod debug;
mod drawing;
mod edit;
mod error;
mod list;
mod mv;
mod new;
mod new_account;
mod print;
mod private_key;
mod remove;
mod rename;
mod selector;
mod status;
mod sync;
mod usage;
mod utils;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "The best place to store and share thoughts.")]
enum Lockbook {
    /// Backup your Lockbook files and structure to the current directory
    Backup,

    /// Commands related to managing premium lockbook subscriptions
    Billing(Billing),

    /// Copy a file from your file system into your Lockbook
    ///
    /// If neither dest or dest_id is provided an interactive selector will be launched.
    Copy {
        /// At-least one filesystem location
        #[structopt(required = true)]
        disk: Vec<PathBuf>,

        /// The path to a folder within lockbook.
        dest: Option<String>,

        /// The id of a folder within lockbook.
        #[structopt(short, long)]
        dest_id: Option<Uuid>,
    },

    /// Open a document for editing
    ///
    /// Open a document for editing in an external editor. The default editor is vim on unix-like
    /// systems and vs-code on windows. The editor can be overridden by using the $LOCKBOOK_EDITOR
    /// environment variable.
    ///
    /// If neither path or id is provided an interactive selector will be launched.
    Edit {
        /// The lockbook location of a document within lockbook
        path: Option<String>,

        /// The id of a document within lockbook
        #[structopt(short, long)]
        id: Option<Uuid>,
    },

    /// Export a drawing as an image
    ///
    /// If neither path or id is provided an interactive selector will be launched.
    Drawing {
        /// Path of the drawing within lockbook
        path: Option<String>,

        /// The id of a drawing within lockbook
        #[structopt(short, long)]
        id: Option<Uuid>,

        /// Format for export format, options are: png, jpeg, bmp, tga, pnm, farbfeld
        format: String,
    },

    /// Import or Export a private key
    PrivateKey {
        /// Import a private key from stdin
        #[structopt(short, long)]
        import: bool,

        /// Export a private key to stdout. If piped, it will print the private key as text. Otherwise, it
        /// will produce a QR code.
        #[structopt(short, long)]
        export: bool,
    },

    /// Prints uncompressed & compressed local disk utilization, and server disk utilization
    GetUsage {
        /// Show machine readable amounts, in bytes
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
    ///
    /// Source & Destination must exist prior to moving.
    /// If neither src or src_id is provided an interactive selector will be launched.
    /// If neither dest or dest_id is provided an interactive selector will be launched.
    Move {
        /// Path of the file within lockbook to move
        src: Option<String>,

        /// Id of the file within lockbook to move
        #[structopt(short, long)]
        src_id: Option<Uuid>,

        /// Path to the desired destination folder within lockbook
        dest: Option<String>,

        /// Id of the desired destination folder within lockbook
        #[structopt(short, long)]
        dest_id: Option<Uuid>,
    },

    /// Create a new document or folder
    ///
    /// Can either provide a path or a parent-id + name.
    /// If neither path, parent or name is provided, an interactive selector will be launched.
    New {
        /// Desired path, folders that don't exist will be created. The terminal file type will be
        /// determined based on whether the last character of the path is a '/' or not
        path: Option<String>,

        /// Id of the parent you're trying to create the file in
        #[structopt(short, long)]
        parent: Option<Uuid>,

        /// Name of the file. file type will be determined based on whether the last character of
        /// the path is a '/' or not
        #[structopt(short, long)]
        name: Option<String>,
    },

    /// Create a new Lockbook account
    NewAccount,

    /// Print the contents of a file to stdout
    ///
    /// If neither path or id is provided an interactive selector will be launched.
    Print {
        /// The lockbook location of a document within lockbook
        path: Option<String>,

        /// The id of a document within lockbook
        #[structopt(short, long)]
        id: Option<Uuid>,
    },

    /// Delete a file
    ///
    /// If neither path or id is provided an interactive selector will be launched.
    Remove {
        /// The lockbook location of a file within lockbook
        path: Option<String>,

        /// The lockbook location of a file within lockbook
        #[structopt(short, long)]
        id: Option<Uuid>,

        /// The id of a file within lockbook
        #[structopt(short, long)]
        force: bool,
    },

    /// Rename a file at a path to a target value
    ///
    /// If neither path or id is provided an interactive selector will be launched.
    /// If name is not provided an interactive selector will be launched.
    Rename {
        /// The lockbook location of a file within lockbook
        path: Option<String>,

        /// The lockbook location of a file within lockbook
        #[structopt(short, long)]
        id: Option<Uuid>,

        /// New name
        name: Option<String>,
    },

    /// What operations a sync would perform
    Status,

    /// Get updates, push changes
    Sync,

    /// Subcommands that aid in extending lockbook
    Debug(Debug),
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum Debug {
    /// Prints metadata associated with a file
    Info {
        path: Option<String>,

        #[structopt(short, long)]
        id: Option<Uuid>,
    },

    /// Prints all the error codes that the cli can generate
    Errors,

    /// Prints who is logged into this lockbook
    WhoAmI,

    /// Prints information about where this lockbook is stored and what server it communicates with
    WhereAmI,

    /// Helps find invalid states within lockbook
    Validate,

    /// Visualizes the filetree as a graphical tree
    Tree,
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum Billing {
    /// Prints out information about your current tier
    Status,

    /// Create a new subscription using a credit card
    Subscribe,

    /// Terminate a lockbook subscription
    UnSubscribe,
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

    use Lockbook::*;
    match Lockbook::from_args() {
        Copy { disk: files, dest, dest_id } => copy::copy(&core, &files, dest, dest_id),
        Billing(billing) => billing::billing(&core, billing),
        Edit { path, id } => edit::edit(&core, path, id),
        PrivateKey { import, export } => private_key::private_key(&core, import, export),
        NewAccount => new_account::new_account(&core),
        List { ids, folders, documents, all } => list::list(&core, ids, documents, folders, all),
        Move { src, src_id, dest, dest_id } => mv::mv(&core, src, src_id, dest, dest_id),
        New { path, parent, name } => new::new(&core, path, parent, name),
        Print { path, id } => print::print(&core, path, id),
        Remove { path, id, force } => remove::remove(&core, path, id, force),
        Rename { path, id, name } => rename::rename(&core, path, id, name),
        Status => status::status(&core),
        Sync => sync::sync(&core),
        Backup => backup::backup(&core),
        GetUsage { exact } => usage::usage(&core, exact),
        Drawing { path, id, format } => drawing::drawing(&core, path, id, &format),
        Debug(debug) => debug::debug(&core, debug),
    }
}

fn main() {
    if let Err(err) = parse_and_run() {
        exit_with(err);
    }
}
