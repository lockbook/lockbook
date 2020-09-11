use crate::model::account::Account;
use crate::repo::account_repo::AccountRepo;
use crate::repo::{account_repo, db_version_repo};
use crate::service::db_state_service::GetStateError::AccountDbError;
use crate::service::db_state_service::State::Empty;
use sled::Db;

#[derive(Debug)]
pub enum State {
    ReadyToUse,
    Empty,
    MigrationRequired,
}

#[derive(Debug)]
pub enum GetStateError {
    AccountDbError(account_repo::DbError),
    RepoError(db_version_repo::Error),
}

#[derive(Debug)]
pub enum MigrationError {
    RepoError(db_version_repo::Error),
}

pub trait DbStateService {
    fn get_state(db: &Db) -> Result<State, GetStateError>;
    fn perform_migration(db: &Db) -> Result<MigrationError, ()>;
}

pub struct DbStateServiceImpl<AccountDb: AccountRepo> {
    _account: AccountDb,
}

impl<AccountDb: AccountRepo> DbStateService for DbStateServiceImpl<AccountDb> {
    fn get_state(db: &Db) -> Result<State, GetStateError> {
        if AccountDb::maybe_get_account(&db)
            .map_err(AccountDbError)?
            .is_none()
        {
            return Ok(Empty);
        }
    }

    fn perform_migration(db: &Db) -> Result<MigrationError, ()> {
        unimplemented!()
    }
}
