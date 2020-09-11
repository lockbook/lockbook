use crate::repo::account_repo::AccountRepo;
use crate::repo::db_version_repo::DbVersionRepo;
use crate::repo::{account_repo, db_version_repo};
use crate::service::db_state_service::GetStateError::{AccountDbError, RepoError};
use crate::service::db_state_service::State::{
    Empty, MigrationRequired, ReadyToUse, StateRequiresClearing,
};
use sled::Db;

#[derive(Debug)]
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
        let code_version = env!("CARGO_PKG_VERSION").to_string();

        if AccountDb::maybe_get_account(&db)
            .map_err(AccountDbError)?
            .is_none()
        {
            VersionDb::set(&db, &code_version).map_err(RepoError)?;
            return Ok(Empty);
        }

        match VersionDb::get(&db).map_err(RepoError)? {
            None => Ok(StateRequiresClearing),
            Some(state_version) => {
                if state_version == code_version {
                    Ok(ReadyToUse)
                } else {
                    Ok(MigrationRequired)
                }
            }
        }
    }

    fn perform_migration(db: &Db) -> Result<(), MigrationError> {
        loop {
            let code_version = env!("CARGO_PKG_VERSION").to_string();

            let db_version = match VersionDb::get(&db).map_err(MigrationError::RepoError)? {
                None => return Err(MigrationError::StateRequiresClearing),
                Some(version) => version,
            };

            if db_version == code_version {
                return Ok(());
            }

            match db_version.as_str() {
                "0.1.0" => VersionDb::set(&db, "0.1.1").map_err(MigrationError::RepoError)?,
                "0.1.1" => return Ok(()),
                _ => return Err(MigrationError::StateRequiresClearing),
            }
        }
    }
}
