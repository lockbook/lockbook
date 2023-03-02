mod account;
mod debug;
mod edit;
mod error;
mod imex;
mod list;
mod share;

use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;

use lb::Core;

use self::error::CliError;

const ID_PREFIX_LEN: usize = 8;

#[derive(Parser, Debug)]
enum LbCli {
    /// account related commands
    #[command(subcommand)]
    Account(account::AccountCmd),
    /// import files from your file system into lockbook
    Copy {
        /// paths of file on disk
        disk_files: Vec<PathBuf>,
        /// lockbook file path or ID destination
        dest: String,
    },
    /// investigative commands
    #[command(subcommand)]
    Debug(debug::DebugCmd),
    /// delete a file
    Delete {
        /// lockbook file path or ID
        target: String,
        /// do not prompt for confirmation before deleting
        force: bool,
    },
    /// edit a document
    Edit {
        /// lockbook file path or ID
        target: String,
    },
    /// export a lockbook file to your file system
    Export {
        /// the path or id of a lockbook folder
        target: String,
        /// a filesystem location (defaults to current directory)
        dest: Option<PathBuf>,
    },
    /// list files and file information
    List(list::ListArgs),
    /// move a file to a new parent
    Move {
        /// lockbook file path or ID of the file to move
        src_target: String,
        /// lockbook file path or ID of the new parent
        dest_target: String,
    },
    /// create a new file at the given path or do nothing if it exists
    New {
        /// lockbook file path
        path: String,
    },
    /// print a document to stdout
    Print {
        /// lockbook file path or ID
        target: String,
    },
    /// rename a file
    Rename {
        /// lockbook file path or ID
        target: String,
        /// the file's new name
        new_name: String,
    },
    /// sharing related commands
    #[command(subcommand)]
    Share(share::ShareCmd),
    /// file sync
    Sync,
}

fn input<T>(prompt: impl fmt::Display) -> Result<T, CliError>
where
    T: FromStr,
    <T as FromStr>::Err: fmt::Debug,
{
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .expect("failed to read from stdin");
    answer.retain(|c| c != '\n' && c != '\r');

    Ok(answer.parse::<T>().unwrap())
}

pub fn maybe_get_by_path(core: &lb::Core, p: &str) -> Result<Option<lb::File>, CliError> {
    match core.get_by_path(p) {
        Ok(f) => Ok(Some(f)),
        Err(err) => match err.kind {
            lb::CoreError::FileNonexistent => Ok(None),
            _ => Err(err.into()),
        },
    }
}

fn resolve_target_to_file(core: &Core, t: &str) -> Result<lb::File, CliError> {
    match lb::Uuid::parse_str(t) {
        Ok(id) => core.get_file_by_id(id).map_err(|err| err.into()),
        Err(_) => core.get_by_path(t).map_err(|err| err.into()),
    }
}

fn resolve_target_to_id(core: &Core, t: &str) -> Result<lb::Uuid, CliError> {
    if let Ok(id) = lb::Uuid::parse_str(t) {
        return Ok(id);
    }
    match core.get_by_path(t) {
        Ok(f) => Ok(f.id),
        Err(err) => Err(err.into()),
    }
}

fn delete(core: &Core, target: &str, force: bool) -> Result<(), CliError> {
    let f = resolve_target_to_file(core, target)?;

    if !force {
        let mut phrase = format!("delete '{}'", target);
        if f.is_folder() {
            let count = core.get_and_get_children_recursively(f.id)?.len();
            phrase = format!("{phrase} and its {count} children")
        }

        let answer: String = input(format!("are you sure you want to {phrase}? [y/n]: "))?;
        if answer != "y" && answer != "Y" {
            println!("aborted.");
            return Ok(());
        }
    }

    core.delete_file(f.id)?;
    Ok(())
}

fn move_file(core: &Core, src: &str, dest: &str) -> Result<(), CliError> {
    let src_id = resolve_target_to_id(core, src)?;
    let dest_id = resolve_target_to_id(core, dest)?;
    core.move_file(src_id, dest_id)
        .map_err(|err| CliError(format!("could not move '{}' to '{}': {:?}", src_id, dest_id, err)))
}

fn print(core: &Core, target: &str) -> Result<(), CliError> {
    let id = resolve_target_to_id(core, target)?;
    let content = core.read_document(id)?;
    print!("{}", String::from_utf8_lossy(&content));
    io::stdout().flush()?;
    Ok(())
}

fn rename(core: &Core, target: &str, new_name: &str) -> Result<(), CliError> {
    let id = resolve_target_to_id(core, target)?;
    core.rename_file(id, new_name)?;
    Ok(())
}

fn sync(core: &Core) -> Result<(), CliError> {
    println!("syncing...");
    core.sync(Some(Box::new(|sp: lb::SyncProgress| {
        use lb::ClientWorkUnit::*;
        match sp.current_work_unit {
            PullMetadata => println!("pulling file tree updates"),
            PushMetadata => println!("pushing file tree updates"),
            PullDocument(name) => println!("pulling: {}", name),
            PushDocument(name) => println!("pushing: {}", name),
        };
    })))?;
    Ok(())
}

fn create(core: &Core, path: &str) -> Result<(), CliError> {
    match core.get_by_path(path) {
        Ok(_f) => Ok(()),
        Err(err) => match err.kind {
            lb::CoreError::FileNonexistent => match core.create_at_path(path) {
                Ok(_f) => Ok(()),
                Err(err) => Err(err.into()),
            },
            _ => Err(err.into()),
        },
    }
}

fn run() -> Result<(), CliError> {
    let writeable_path = match (std::env::var("LOCKBOOK_PATH"), std::env::var("HOME")) {
        (Ok(s), _) => s,
        (Err(_), Ok(s)) => format!("{}/.lockbook/cli", s),
        _ => return Err(CliError::new("no cli location")),
    };

    let core = Core::init(&lb::Config { writeable_path, logs: true, colored_logs: true })?;

    let cmd = LbCli::parse();
    if !matches!(cmd, LbCli::Account(account::AccountCmd::New { .. }))
        && !matches!(cmd, LbCli::Account(account::AccountCmd::Import))
    {
        let _ = core.get_account().map_err(|err| match err.kind {
            lb::CoreError::AccountNonexistent => {
                CliError::new("no account! run 'init' or 'init --restore' to get started.")
            }
            _ => err.into(),
        })?;
    }

    match cmd {
        LbCli::Account(cmd) => account::account(&core, cmd),
        LbCli::Copy { disk_files, dest } => imex::copy(&core, &disk_files, &dest),
        LbCli::Debug(cmd) => debug::debug(&core, cmd),
        LbCli::Delete { target, force } => delete(&core, &target, force),
        LbCli::Edit { target } => edit::edit(&core, &target),
        LbCli::Export { target, dest } => imex::export(&core, &target, dest),
        LbCli::List(args) => list::list(&core, args),
        LbCli::Move { src_target, dest_target } => move_file(&core, &src_target, &dest_target),
        LbCli::New { path } => create(&core, &path),
        LbCli::Print { target } => print(&core, &target),
        LbCli::Rename { target, new_name } => rename(&core, &target, &new_name),
        LbCli::Share(cmd) => share::share(&core, cmd),
        LbCli::Sync => sync(&core),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        std::process::exit(1)
    }
}
