mod import;
mod local_sync;

use clap::Parser;
use lb::Core;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about)]
enum Command {
    LocalSync { location: PathBuf },
    Import,
}

struct Drive {
    c: Core,
}

fn main() {
    let c = core();
    let drive = Drive { c };

    let cmd = Command::parse();
    match cmd {
        Command::Import => drive.import(),
        Command::LocalSync { location } => drive.check_for_changes(location),
    }
}

fn core() -> Core {
    let writeable_path = format!("{}/.lockbook/drive", std::env::var("HOME").unwrap());

    Core::init(&lb::Config { writeable_path, logs: true, colored_logs: true }).unwrap()
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
