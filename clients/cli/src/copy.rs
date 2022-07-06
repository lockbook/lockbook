use std::cell::Cell;
use std::io::Write;
use std::path::PathBuf;

use lockbook_core::Core;
use lockbook_core::CoreError;
use lockbook_core::Error as LbError;
use lockbook_core::FileType::Folder;
use lockbook_core::ImportFileError;
use lockbook_core::ImportStatus;

use crate::error::CliError;
use crate::selector::select_meta;
use crate::Uuid;

pub fn copy(
    core: &Core, disk_paths: &[PathBuf], lb_path: Option<String>, dest_id: Option<Uuid>,
) -> Result<(), CliError> {
    core.get_account()?;

    let total = Cell::new(0);
    let nth_file = Cell::new(0);
    let update_status = move |status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n_files) => total.set(n_files),
        ImportStatus::Error(disk_path, err) => match err {
            CoreError::DiskPathInvalid => eprintln!("invalid disk path '{}'", disk_path.display()),
            _ => eprintln!("unexpected error: {:#?}", err),
        },
        ImportStatus::StartingItem(disk_path) => {
            nth_file.set(nth_file.get() + 1);
            print!("({}/{}) Importing: {}... ", nth_file.get(), total.get(), disk_path);
            std::io::stdout().flush().unwrap();
        }
        ImportStatus::FinishedItem(_meta) => println!("Done."),
    };

    let dest =
        select_meta(core, lb_path, dest_id, Some(Folder), Some("Select an import destination"))?;
    let dest_path = core.get_path_by_id(dest.id)?;

    core.import_files(disk_paths, dest.id, &update_status)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                ImportFileError::ParentDoesNotExist => CliError::file_not_found(dest_path),
                ImportFileError::DocumentTreatedAsFolder => CliError::doc_treated_as_dir(dest_path),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
}
