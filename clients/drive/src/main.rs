use clap::Parser;
use std::{fs, path::PathBuf};

extern crate notify;

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Config};
use std::sync::mpsc::channel;
use std::time::Duration;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(version, about)]
enum Command {
    LocalSync{
        location: PathBuf,
    }
}

fn main(){
    let cmd = Command::parse();
    match cmd {
        Command::LocalSync{location} => {
            println!("{:?}", location);
            check_for_changes().unwrap();
        }
    }
}

fn check_for_changes() -> notify::Result<()>{
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
