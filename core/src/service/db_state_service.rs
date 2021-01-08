use crate::repo::account_repo::AccountRepo;
use crate::repo::db_version_repo::DbVersionRepo;
use crate::repo::{account_repo, db_version_repo};
use crate::service::code_version_service::CodeVersion;
use crate::service::db_state_service::State::{
    Empty, MigrationRequired, ReadyToUse, StateRequiresClearing,
};
use crate::storage::db_provider::Backend;
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
pub enum State {
    ReadyToUse,
    Empty,
    MigrationRequired,
    StateRequiresClearing,
}

#[derive(Debug)]
pub enum GetStateError<MyBackend: Backend> {
    AccountRepoError(account_repo::AccountRepoError<MyBackend>),
    RepoError(db_version_repo::Error<MyBackend>),
}

#[derive(Debug)]
pub enum MigrationError<MyBackend: Backend> {
    StateRequiresClearing,
    RepoError(db_version_repo::Error<MyBackend>),
}

pub trait DbStateService<MyBackend: Backend> {
    fn get_state(backend: &MyBackend::Db) -> Result<State, GetStateError<MyBackend>>;
    fn perform_migration(backend: &MyBackend::Db) -> Result<(), MigrationError<MyBackend>>;
}

pub struct DbStateServiceImpl<
    AccountDb: AccountRepo<MyBackend>,
    VersionDb: DbVersionRepo<MyBackend>,
    Version: CodeVersion,
    MyBackend: Backend,
> {
    _account: AccountDb,
    _repo: VersionDb,
    _version: Version,
    _backend: MyBackend,
}

impl<
        AccountDb: AccountRepo<MyBackend>,
        VersionDb: DbVersionRepo<MyBackend>,
        Version: CodeVersion,
        MyBackend: Backend,
    > DbStateService<MyBackend> for DbStateServiceImpl<AccountDb, VersionDb, Version, MyBackend>
{
    fn get_state(backend: &MyBackend::Db) -> Result<State, GetStateError<MyBackend>> {
        if AccountDb::maybe_get_account(backend)
            .map_err(GetStateError::AccountRepoError)?
            .is_none()
        {
            VersionDb::set(backend, Version::get_code_version())
                .map_err(GetStateError::RepoError)?;
            return Ok(Empty);
        }

        match VersionDb::get(backend).map_err(GetStateError::RepoError)? {
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

    fn perform_migration(backend: &MyBackend::Db) -> Result<(), MigrationError<MyBackend>> {
        loop {
            let db_version = match VersionDb::get(backend).map_err(MigrationError::RepoError)? {
                None => return Err(MigrationError::StateRequiresClearing),
                Some(version) => version,
            };

            if db_version == Version::get_code_version() {
                return Ok(());
            }

            match db_version.as_str() {
                "0.1.0" => VersionDb::set(backend, "0.1.1").map_err(MigrationError::RepoError)?,
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
    use crate::repo::db_version_repo::{DbVersionRepo, DbVersionRepoImpl};
    use crate::service::code_version_service::{CodeVersion, CodeVersionImpl};
    use crate::service::db_state_service::DbStateService;
    use crate::service::db_state_service::State::Empty;
    use crate::storage::db_provider::{Backend, FileBackend};
    use crate::DefaultDbStateService;

    #[test]
    fn test_initial_state() {
        let config = temp_config();
        let backend = FileBackend::connect_to_db(&config).unwrap();

        assert!(DbVersionRepoImpl::get(backend).unwrap().is_none());
        assert_eq!(DefaultDbStateService::get_state(backend).unwrap(), Empty);
        assert_eq!(DefaultDbStateService::get_state(backend).unwrap(), Empty);
        assert_eq!(
            DbVersionRepoImpl::get(backend).unwrap().unwrap(),
            CodeVersionImpl::get_code_version()
        );
    }

    // The rest are integration tests
}
