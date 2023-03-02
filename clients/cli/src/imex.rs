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

    let dest = if let Some(path) = maybe_dest {
        path
    } else {
        // If no destination path is provided, it'll be a file with the target name in the current
        // directory. If it's root, it'll be the account's username.
        let name = if target_file.id == target_file.parent {
            core.get_account()?.username
        } else {
            target_file.name.clone()
        };
        let mut dir = env::current_dir()?;
        dir.push(name);
        dir
    };

    println!("exporting '{}'...", target_file.name);
    fs::create_dir(&dest)?;

    core.export_file(target_file.id, dest, false, None)?;
    Ok(())
}

pub fn copy(core: &lb::Core, disk_files: &[PathBuf], dest: &str) -> Result<(), CliError> {
    let dest_id = resolve_target_to_id(core, dest)?;

    let total = Cell::new(0);
    let nth_file = Cell::new(0);
    let update_status = move |status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n_files) => total.set(n_files),
        ImportStatus::Error(disk_path, err) => match err {
            lb::CoreError::DiskPathInvalid => {
                eprintln!("invalid disk path '{}'", disk_path.display())
            }
            _ => eprintln!("unexpected error: {:#?}", err),
        },
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
