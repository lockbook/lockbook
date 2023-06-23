use clap::Parser;
use is_terminal::IsTerminal;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

//extern crate notify;

use notify::EventKind;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
//use std::time::Duration;
//use lockbook_core::CoreLib;
//use lockbook_shared;

use lb::Core;
//use self::error::CliError;

#[derive(Parser, Debug)]
#[command(version, about)]
enum Command {
    LocalSync { location: PathBuf },
    Import,
}

fn main() {
    let c = core();
    let cmd = Command::parse();
    match cmd {
        Command::Import => import_acct(&c),
        Command::LocalSync { location } => {
            c.get_account().unwrap();
            println!("{:?}", location);
            check_for_changes(&c, location).unwrap();
        }
    }
}

fn import_acct(core: &Core) {
    if std::io::stdin().is_terminal() {
        panic!("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | drive import");
    }

    let mut account_string = String::new();
    std::io::stdin()
        .read_line(&mut account_string)
        .expect("failed to read from stdin");
    account_string.retain(|c| !c.is_whitespace());

    println!("importing account...");
    core.import_account(&account_string).unwrap();

    println!("account imported!");
}

fn core() -> Core {
    let writeable_path = format!("{}/.lockbook/drive", std::env::var("HOME").unwrap());

    Core::init(&lb::Config { writeable_path, logs: true, colored_logs: true }).unwrap()
}

fn check_for_changes(core: &Core, mut dest: PathBuf) -> notify::Result<()> {
    //sync(&core).unwrap();
    core.sync(Some(Box::new(|sp: lb::SyncProgress| {
        use lb::ClientWorkUnit::*;
        match sp.current_work_unit {
            PullMetadata => println!("pulling file tree updates"),
            PushMetadata => println!("pushing file tree updates"),
            PullDocument(f) => println!("pulling: {}", f.name),
            PushDocument(f) => println!("pushing: {}", f.name),
        };
    })))
    .unwrap();

    core.export_file(core.get_root().unwrap().id, dest.clone(), false, None)
        .unwrap();
    File::open(&dest).unwrap().sync_all().unwrap();
    // Create a channel to receive file events
    let (tx, rx) = channel();
    //let filepath: &str = "example.txt";
    dest.push(core.get_root().unwrap().name);
    dest = dest.canonicalize().unwrap();
    // Create a new watcher object
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    // Register the file for watching
    watcher.watch(&dest, RecursiveMode::Recursive)?;

    println!("Watching for changes in {:?}", dest);

    for res in rx {
        match res {
            Ok(event) => {
                println!("{:#?}", event);
                match event.kind {
                    EventKind::Any => {},
                    EventKind::Access(_) => {},
                    EventKind::Create(_) => {},
                    EventKind::Modify(_) => {},
                    EventKind::Remove(_) => {
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let to_delete = core.get_by_path(core_path.to_str().unwrap()).unwrap();
                        core.delete_file(to_delete.id).unwrap();
                    }
                    EventKind::Other => {},
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}

fn get_lockbook_path(event_path: PathBuf, dest: PathBuf) -> PathBuf {
    let mut ep_iter = event_path.iter();
    for _ in &dest {
        ep_iter.next();
    }
    PathBuf::from(&ep_iter)
}

#[test]
fn test() {
    let a = PathBuf::from("/a/b/c/d");
    let b = PathBuf::from("/a/b/c");
    let c = get_lockbook_path(a, b);
    assert_eq!(c, PathBuf::from("d"));
}

#[test]
fn test2() {
    let a = PathBuf::from(
        "/Users/siddhantsapra/Desktop/lockbook/lockbook/clients/drive/siddhant/test.md",
    );
    let b = PathBuf::from("/a/b/c");
    let c = get_lockbook_path(a, b);
    assert_eq!(c, PathBuf::from("d"));
}
// Step 1: Either make sure directory is empty or empty it
// ✅Step 2: Initialize a core - Core::Init pointed at .lockbook, location = .lockbook/drive
// ✅Step 3: Determine whether they are signed in - core.get_account
// ✅Step 4: Initially they shouldn't be signed in - provide another subcommand where they can import their account
// ✅Step 4.5: If you import an account rn, call core.sync
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

// In check for changes, lockbook.sync, lockbook export, watch for changes, and replicate something
// If renamed, differentiate between different possible outcomes
// Try differentiation plus create, changed or deleted (rm) handled - core.create, core.delete, core.write
// Might have some level of path calculation at times
// Does the linux api expose more information?
