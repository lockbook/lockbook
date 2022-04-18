use uuid::Uuid;

use lockbook_core::model::errors::{TestRepoError, Warning};
use lockbook_core::LbCore;

use crate::error::CliError;

pub fn validate(core: &LbCore) -> Result<(), CliError> {
    core.get_account()?;

    let err = match core.validate() {
        Ok(warnings) => {
            if warnings.is_empty() {
                return Ok(());
            };

            for w in &warnings {
                match w {
                    Warning::EmptyFile(id) => {
                        let path = get_path_by_id_or_err(core, *id)?;
                        eprintln!("File at path {} is empty.", path);
                    }
                    Warning::InvalidUTF8(id) => {
                        let path = get_path_by_id_or_err(core, *id)?;
                        eprintln!("File at path {} contains invalid UTF8.", path);
                    }
                    Warning::UnreadableDrawing(id) => {
                        let path = get_path_by_id_or_err(core, *id)?;
                        eprintln!("Drawing at path {} is unreadable.", path);
                    }
                }
            }

            CliError::validate_warnings_found(warnings.len())
        }
        Err(err) => match err {
            TestRepoError::NoAccount => CliError::no_account(),
            TestRepoError::NoRootFolder => CliError::no_root(),
            TestRepoError::DocumentTreatedAsFolder(id) => {
                CliError::doc_treated_as_dir(get_path_by_id_or_err(core, id)?)
            }
            TestRepoError::FileOrphaned(id) => {
                CliError::file_orphaned(get_path_by_id_or_err(core, id)?)
            }
            TestRepoError::CycleDetected(_) => CliError::cycle_detected(),
            TestRepoError::FileNameEmpty(_) => CliError::file_name_empty(),
            TestRepoError::FileNameContainsSlash(id) => {
                CliError::file_name_has_slash(get_path_by_id_or_err(core, id)?)
            }
            TestRepoError::NameConflictDetected(id) => {
                CliError::name_conflict_detected(get_path_by_id_or_err(core, id)?)
            }
            TestRepoError::DocumentReadError(id, err) => {
                CliError::validate_doc_read(get_path_by_id_or_err(core, id)?, format!("{:#?}", err))
            }
            TestRepoError::Core(err) => {
                CliError::unexpected(format!("unexpected error: {:#?}", err))
            }
            TestRepoError::Tree(err) => {
                CliError::unexpected(format!("unexpected error: {:#?}", err))
            }
        },
    };

    Err(err)
}

fn get_path_by_id_or_err(core: &LbCore, id: Uuid) -> Result<String, CliError> {
    core.get_path_by_id(id).map_err(|err| {
        CliError::unexpected(format!("failed to get path by id: {} err: {:#?}", id, err))
    })
}
