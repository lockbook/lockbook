use sled::Db;

use crate::repo::account_repo::AccountRepo;
use crate::repo::db_version_repo::DbVersionRepo;
use crate::repo::{account_repo, db_version_repo};
use crate::service::db_state_service::GetStateError::{AccountDbError, RepoError};
use crate::service::db_state_service::State::{Empty, ReadyToUse, StateRequiresClearing};
use crate::CORE_CODE_VERSION;

#[derive(Debug, PartialEq)]
pub enum State {
    ReadyToUse,
    Empty,
    MigrationRequired,
    StateRequiresClearing,
}

#[derive(Debug)]
pub enum GetStateError {
    AccountDbError(account_repo::DbError),
    RepoError(db_version_repo::Error),
}

#[derive(Debug)]
pub enum MigrationError {
    StateRequiresClearing,
    RepoError(db_version_repo::Error),
}

pub trait DbStateService {
    fn get_state(db: &Db) -> Result<State, GetStateError>;
    fn perform_migration(db: &Db) -> Result<(), MigrationError>;
}

pub struct DbStateServiceImpl<AccountDb: AccountRepo, VersionDb: DbVersionRepo> {
    _account: AccountDb,
    _repo: VersionDb,
}

impl<AccountDb: AccountRepo, VersionDb: DbVersionRepo> DbStateService
    for DbStateServiceImpl<AccountDb, VersionDb>
{
    fn get_state(db: &Db) -> Result<State, GetStateError> {
        if AccountDb::maybe_get_account(&db)
            .map_err(AccountDbError)?
            .is_none()
        {
            VersionDb::set(&db, CORE_CODE_VERSION).map_err(RepoError)?;
            return Ok(Empty);
        }

        match VersionDb::get(&db).map_err(RepoError)? {
            None => Ok(StateRequiresClearing),
            Some(state_version) => {
                if state_version == CORE_CODE_VERSION {
                    Ok(ReadyToUse)
                } else {
                    match state_version.as_str() {
                        "0.1.0" => Ok(StateRequiresClearing),
                        "0.1.1" => Ok(StateRequiresClearing),
                        "0.1.2" => Ok(ReadyToUse),
                        _ => Ok(StateRequiresClearing),
                    }
                }
            }
        }
    }

    fn perform_migration(db: &Db) -> Result<(), MigrationError> {
        loop {
            let db_version = match VersionDb::get(&db).map_err(MigrationError::RepoError)? {
                None => return Err(MigrationError::StateRequiresClearing),
                Some(version) => version,
            };

            if db_version == CORE_CODE_VERSION {
                return Ok(());
            }

            match db_version.as_str() {
                "0.1.0" => VersionDb::set(&db, "0.1.1").map_err(MigrationError::RepoError)?,
                "0.1.1" => return Err(MigrationError::StateRequiresClearing), // If you wanted to remove this, write a migration for PR #332
                "0.1.2" => return Ok(()),
                _ => return Err(MigrationError::StateRequiresClearing),
            };
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::dummy_config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::db_version_repo::{DbVersionRepo, DbVersionRepoImpl};
    use crate::service::db_state_service::DbStateService;
    use crate::service::db_state_service::State::Empty;
    use crate::{DefaultDbStateService, CORE_CODE_VERSION};

    #[test]
    fn test_initial_state() {
        let config = dummy_config();
        let db = TempBackedDB::connect_to_db(&config).unwrap();

        assert!(DbVersionRepoImpl::get(&db).unwrap().is_none());
        assert_eq!(DefaultDbStateService::get_state(&db).unwrap(), Empty);
        assert_eq!(DefaultDbStateService::get_state(&db).unwrap(), Empty);
        assert_eq!(
            DbVersionRepoImpl::get(&db).unwrap().unwrap(),
            CORE_CODE_VERSION
        );
    }

    // The rest are integration tests
}
