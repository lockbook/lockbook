use std::{
    cell::Cell,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use cli_rs::cli_error::CliResult;
use lb::{Core, ImportStatus};

use crate::{ensure_account_and_root, input::FileInput};

pub fn copy(core: &Core, disk: PathBuf, parent: FileInput) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let parent = parent.find(core)?.id;

    let total = Cell::new(0);
    let nth_file = Cell::new(0);
    let update_status = move |status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n_files) => total.set(n_files),
        ImportStatus::StartingItem(disk_path) => {
            nth_file.set(nth_file.get() + 1);
            print!("({}/{}) importing: {}... ", nth_file.get(), total.get(), disk_path);
            io::stdout().flush().unwrap();
        }
        ImportStatus::FinishedItem(_meta) => println!("done."),
    };

    core.import_files(&[disk], parent, &update_status)?;

    Ok(())
}

pub fn export(core: &Core, target: FileInput, dest: PathBuf) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let target_file = target.find(core)?;

    println!("exporting '{}'...", target_file.name);
    if !dest.exists() {
        fs::create_dir(&dest)?;
    }

    core.export_file(target_file.id, dest, false, None)?;
    Ok(())
}
