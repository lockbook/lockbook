use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config};
use crate::{err, err_unexpected};
use lockbook_core::get_path_by_id;
use lockbook_core::model::state::Config;
use lockbook_core::service::integrity_service::{test_repo_integrity, TestRepoError, Warning};
use uuid::Uuid;

pub fn validate() -> CliResult<()> {
    get_account_or_exit();

    let config = get_config();

    let err = match test_repo_integrity(&config, false) {
        Ok(warnings) => {
            if warnings.len() == 0 {
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
            TestRepoError::Core(err) => {
                err_unexpected!("an unexpected error occurred: {:#?}", err)
            }
        },
    };

    Err(err)
}

fn get_path_by_id_or_err(config: &Config, id: Uuid) -> CliResult<String> {
    get_path_by_id(config, id)
        .map_err(|err| err_unexpected!("failed to get path by id: {} err: {:#?}", id, err))
}
