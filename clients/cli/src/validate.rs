use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};
use lockbook_core::get_path_by_id;
use lockbook_core::model::state::Config;
use lockbook_core::service::integrity_service::{test_repo_integrity, TestRepoError, Warning};
use uuid::Uuid;

pub fn validate() -> CliResult<()> {
    account()?;

    let config = config()?;

    let err = match test_repo_integrity(&config) {
        Ok(warnings) => {
            if warnings.is_empty() {
                return Ok(());
            };

            for w in &warnings {
                match w {
                    Warning::EmptyFile(id) => {
                        let path = get_path_by_id_or_err(&config, *id)?;
                        eprintln!("File at path {} is empty.", path);
                    }
                    Warning::InvalidUTF8(id) => {
                        let path = get_path_by_id_or_err(&config, *id)?;
                        eprintln!("File at path {} contains invalid UTF8.", path);
                    }
                    Warning::UnreadableDrawing(id) => {
                        let path = get_path_by_id_or_err(&config, *id)?;
                        eprintln!("Drawing at path {} is unreadable.", path);
                    }
                }
            }
            err!(WarningsFound(warnings.len() as i32))
        }
        Err(err) => match err {
            TestRepoError::NoRootFolder => err!(NoRootOps("validate")),
            TestRepoError::DocumentTreatedAsFolder(id) => {
                err!(DocTreatedAsFolder(get_path_by_id_or_err(&config, id)?))
            }
            TestRepoError::FileOrphaned(id) => {
                err!(FileOrphaned(get_path_by_id_or_err(&config, id)?))
            }
            TestRepoError::CycleDetected(_) => {
                err!(CycleDetected)
            }
            TestRepoError::FileNameEmpty(_) => {
                err!(FileNameEmpty)
            }
            TestRepoError::FileNameContainsSlash(id) => {
                err!(FileNameHasSlash(get_path_by_id_or_err(&config, id)?))
            }
            TestRepoError::NameConflictDetected(id) => {
                err!(NameConflictDetected(get_path_by_id_or_err(&config, id)?))
            }
            TestRepoError::DocumentReadError(id, err) => {
                err!(DocumentReadError(
                    get_path_by_id_or_err(&config, id)?,
                    format!("{:#?}", err)
                ))
            }
            TestRepoError::Core(err) | TestRepoError::Tree(err) => {
                err_unexpected!("an unexpected error occurred: {:#?}", err)
            }
            TestRepoError::NoAccount => err!(NoAccount),
        },
    };

    Err(err)
}

fn get_path_by_id_or_err(config: &Config, id: Uuid) -> CliResult<String> {
    get_path_by_id(config, id)
        .map_err(|err| err_unexpected!("failed to get path by id: {} err: {:#?}", id, err))
}
