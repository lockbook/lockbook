use std::cell::Cell;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use lb::ImportStatus;

use crate::resolve_target_to_file;
use crate::resolve_target_to_id;
use crate::CliError;

pub fn export(core: &lb::Core, target: &str, maybe_dest: Option<PathBuf>) -> Result<(), CliError> {
    let target_file = resolve_target_to_file(core, target)?;

    let dest = match maybe_dest {
        Some(path) => path,
        None => env::current_dir()?,
    };

    println!("exporting '{}'...", target_file.name);
    if !dest.exists() {
        fs::create_dir(&dest)?;
    }

    core.export_file(target_file.id, dest, false, None)?;
    Ok(())
}

pub fn copy(core: &lb::Core, disk_files: &[PathBuf], dest: &str) -> Result<(), CliError> {
    let dest_id = resolve_target_to_id(core, dest)?;

    let total = Cell::new(0);
    let nth_file = Cell::new(0);
    let update_status = move |status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n_files) => total.set(n_files),
        ImportStatus::StartingItem(disk_path) => {
            nth_file.set(nth_file.get() + 1);
            print!("({}/{}) importing: {}... ", nth_file.get(), total.get(), disk_path);
            std::io::stdout().flush().unwrap();
        }
        ImportStatus::FinishedItem(_meta) => println!("done."),
    };

    core.import_files(disk_files, dest_id, &update_status)?;
    Ok(())
}
