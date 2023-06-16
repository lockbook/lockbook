use clap::Parser;
use std::path::{PathBuf, Path};

//extern crate notify;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
//use std::time::Duration;

use lb::Core;
//use self::error::CliError;

#[derive(Parser, Debug)]
#[command(version, about)]
enum Command {
    LocalSync { location: PathBuf },
}

fn main() {
    core();
    let cmd = Command::parse();
    match cmd {
        Command::LocalSync { location } => {
            println!("{:?}", location);
            check_for_changes().unwrap();
        }
    }
}

fn core() -> Core{

    let writeable_path = format!("{}/.lockbook/drive", std::env::var("HOME").unwrap());

    Core::init(&lb::Config { writeable_path, logs: true, colored_logs: true }).unwrap()

}

fn check_for_changes() -> notify::Result<()> {
    // Create a channel to receive file events
    let (tx, rx) = channel();
    let filepath: &str = "example.txt";
    // Create a new watcher object
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    // Register the file for watching
    watcher.watch(Path::new(filepath), RecursiveMode::Recursive)?;

    println!("Watching for changes in {filepath}");

    for res in rx {
        match res {
            Ok(event) => println!("changed: {:?}", event),
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}


// Step 1: Either make sure directory is empty or empty it
            // Step 2: Initialize a core - Core::Init pointed at .lockbook, location = .lockbook/drive
            // Step 3: Determine whether they are signed in - core.get_account
            // Step 4: Initially they shouldn't be signed in - provide another subcommand where they can import their account
            // Step 4.5: If you import an account rn, call core.sync
            // Step 5: Write them to the user specified location
            // Step 6: On this user specified location, watch for changes
            // Step 7: Based on what happens, do the corresponding thing inside core (eg if they create file, call core.create)
            // If updated, read contents of core and write the contents to the corresponding place in core
            // Step 8: Call core.sync - should give summary of what happened - apply those changes from core back onto file system
            // When you make changes, how do you know how to ignore them (maybe you just wait for errors and see if they need to be ignored)
            // Alternatively, when they are reduplicated, stop watching the file (lock directory), make changes, and then unlock it
            // Source ID - maybe know your own process ID and ignore it
            // Step 9: To make this safe, determine how to lock the file system so that no one can make changes when the file system is not being watched
            // Naive approach: detect shutdown, delete everything
            // Better approach: Put the file system into a read only state when we shutdown
            // Step 10: Work on the UI