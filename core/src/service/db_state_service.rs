use serde::Serialize;

use crate::model::state::Config;
use crate::repo::{account_repo, db_version_repo};
use crate::CoreError;

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
    if account_repo::maybe_get(config)?.is_none() {
        db_version_repo::set(config, get_code_version())?;
        info!("db_state: empty");
        return Ok(State::Empty);
    }

    let state = match db_version_repo::maybe_get(config)? {
        None => Ok(State::StateRequiresClearing),
        Some(state_version) => {
            if state_version == get_code_version() {
                Ok(State::ReadyToUse)
            } else {
                match state_version.as_str() {
                    "0.1.5" => Ok(State::ReadyToUse),
                    _ => Ok(State::StateRequiresClearing),
                }
            }
        }
    };

    info!("db_state: {:?}", state);
    state
}

pub fn perform_migration(config: &Config) -> Result<(), CoreError> {
    let db_version = match db_version_repo::maybe_get(config)? {
        None => return Err(CoreError::ClientWipeRequired),
        Some(version) => version,
    };

    if db_version == get_code_version() {
        return Ok(());
    }

    match db_version.as_str() {
        "0.1.6" => Ok(()),
        _ => Err(CoreError::ClientWipeRequired),
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::{account_repo, db_version_repo};
    use crate::service::db_state_service::State;
    use crate::service::{db_state_service, test_utils};

    #[test]
    fn get_state_empty() {
        let config = &temp_config();

        assert_eq!(db_state_service::get_state(config).unwrap(), State::Empty);
    }

    #[test]
    fn get_state_ready_to_use() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        db_version_repo::set(config, db_state_service::get_code_version()).unwrap();

        assert_eq!(db_state_service::get_state(config).unwrap(), State::ReadyToUse);
    }

    #[test]
    fn get_state_requires_clearing() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        db_version_repo::set(config, "-1.0.0").unwrap();

        assert_eq!(db_state_service::get_state(config).unwrap(), State::StateRequiresClearing);
    }

    #[test]
    fn perform_migration_empty() {
        let config = &temp_config();

        let result = db_state_service::perform_migration(config);
        assert!(result.is_err());
    }

    #[test]
    fn perform_migration_ready_to_use() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        db_version_repo::set(config, db_state_service::get_code_version()).unwrap();

        let result = db_state_service::perform_migration(config);
        assert!(result.is_ok());
    }

    #[test]
    fn perform_migration_get_state_requires_clearing() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();
        db_version_repo::set(config, "-1.0.0").unwrap();

        let result = db_state_service::perform_migration(config);
        assert!(result.is_err());
    }
}
