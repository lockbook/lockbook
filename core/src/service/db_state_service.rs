use crate::model::state::Config;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_version_repo::DbVersionRepo;
use crate::repo::{account_repo, db_version_repo};
use crate::service::code_version_service::CodeVersion;
use crate::service::db_state_service::State::{
    Empty, MigrationRequired, ReadyToUse, StateRequiresClearing,
};
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
pub enum State {
    ReadyToUse,
    Empty,
    MigrationRequired,
    StateRequiresClearing,
}

#[derive(Debug)]
pub enum GetStateError {
    AccountRepoError(account_repo::AccountRepoError),
    RepoError(db_version_repo::Error),
}

#[derive(Debug)]
pub enum MigrationError {
    StateRequiresClearing,
    RepoError(db_version_repo::Error),
}

pub trait DbStateService {
    fn get_state(config: &Config) -> Result<State, GetStateError>;
    fn perform_migration(config: &Config) -> Result<(), MigrationError>;
}

pub struct DbStateServiceImpl<
    AccountDb: AccountRepo,
    VersionDb: DbVersionRepo,
    Version: CodeVersion,
> {
    _account: AccountDb,
    _repo: VersionDb,
    _version: Version,
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

impl<AccountDb: AccountRepo, VersionDb: DbVersionRepo, Version: CodeVersion> DbStateService
    for DbStateServiceImpl<AccountDb, VersionDb, Version>
{
    fn get_state(config: &Config) -> Result<State, GetStateError> {
        if AccountDb::maybe_get_account(config)
            .map_err(GetStateError::AccountRepoError)?
            .is_none()
        {
            VersionDb::set(config, Version::get_code_version())
                .map_err(GetStateError::RepoError)?;
            return Ok(Empty);
        }

        match VersionDb::get(config).map_err(GetStateError::RepoError)? {
            None => Ok(StateRequiresClearing),
            Some(state_version) => {
                if state_version == Version::get_code_version() {
                    Ok(ReadyToUse)
                } else {
                    match state_version.as_str() {
                        "0.1.0" => Ok(MigrationRequired),
                        "0.1.1" => Ok(StateRequiresClearing),
                        "0.1.2" => Ok(ReadyToUse),
                        _ => Ok(StateRequiresClearing),
                    }
                }
            }
        }
    }

    fn perform_migration(config: &Config) -> Result<(), MigrationError> {
        loop {
            let db_version = match VersionDb::get(config).map_err(MigrationError::RepoError)? {
                None => return Err(MigrationError::StateRequiresClearing),
                Some(version) => version,
            };

            if db_version == Version::get_code_version() {
                return Ok(());
            }

            match db_version.as_str() {
                "0.1.0" => VersionDb::set(config, "0.1.1").map_err(MigrationError::RepoError)?,
                "0.1.1" => return Err(MigrationError::StateRequiresClearing), // If you wanted to remove this, write a migration for PR #332
                "0.1.2" => return Ok(()),
                _ => return Err(MigrationError::StateRequiresClearing),
            };
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::db_version_repo::DbVersionRepo;
    use crate::service::code_version_service::{CodeVersion, CodeVersionImpl};
    use crate::service::db_state_service::DbStateService;
    use crate::service::db_state_service::State::Empty;
    use crate::{DefaultDbStateService, DefaultDbVersionRepo};

    #[test]
    fn test_initial_state() {
        let config = temp_config();

        assert!(DefaultDbVersionRepo::get(&config).unwrap().is_none());
        assert_eq!(DefaultDbStateService::get_state(&config).unwrap(), Empty);
        assert_eq!(DefaultDbStateService::get_state(&config).unwrap(), Empty);
        assert_eq!(
            DefaultDbVersionRepo::get(&config).unwrap().unwrap(),
            CodeVersionImpl::get_code_version()
        );
    }

    // The rest are integration tests
}
