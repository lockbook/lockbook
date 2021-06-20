use crate::model::state::Config;
use crate::repo::{account_repo, db_version_repo};
use crate::service::db_state_service;
use crate::service::db_state_service::State::{Empty, ReadyToUse, StateRequiresClearing};
use crate::CoreError;
use serde::Serialize;

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, PartialEq, Serialize)]
pub enum State {
    ReadyToUse,
    Empty,
    MigrationRequired,
    StateRequiresClearing,
}

pub fn get_state(config: &Config) -> Result<State, CoreError> {
    if account_repo::maybe_get_account(config)?.is_none() {
        db_version_repo::set(config, db_state_service::get_code_version())?;
        return Ok(Empty);
    }

    match db_version_repo::get(config)? {
        None => Ok(StateRequiresClearing),
        Some(state_version) => {
            if state_version == db_state_service::get_code_version() {
                Ok(ReadyToUse)
            } else {
                match state_version.as_str() {
                    "0.1.4" => Ok(ReadyToUse),
                    _ => Ok(StateRequiresClearing),
                }
            }
        }
    }
}

pub fn perform_migration(config: &Config) -> Result<(), CoreError> {
    let db_version = match db_version_repo::get(config)? {
        None => return Err(CoreError::ClientWipeRequired),
        Some(version) => version,
    };

    if db_version == db_state_service::get_code_version() {
        return Ok(());
    }

    match db_version.as_str() {
        "0.1.4" => Ok(()),
        _ => Err(CoreError::ClientWipeRequired),
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::db_version_repo;
    use crate::service::db_state_service;
    use crate::service::db_state_service::State::Empty;

    #[test]
    fn test_initial_state() {
        let config = temp_config();

        assert!(db_version_repo::get(&config).unwrap().is_none());
        assert_eq!(db_state_service::get_state(&config).unwrap(), Empty);
        assert_eq!(db_state_service::get_state(&config).unwrap(), Empty);
        assert_eq!(
            db_version_repo::get(&config).unwrap().unwrap(),
            db_state_service::get_code_version()
        );
    }

    // The rest are integration tests
}
