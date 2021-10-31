use crate::model::state::Config;
use crate::repo::local_storage;
use crate::{core_err_unexpected, CoreError};

static LAST_UPDATED: &[u8; 12] = b"last_updated";

pub fn set(config: &Config, last_updated: u64) -> Result<(), CoreError> {
    debug!("Setting last updated to: {}", last_updated);
    local_storage::write(
        config,
        LAST_UPDATED,
        LAST_UPDATED,
        bincode::serialize(&last_updated).map_err(core_err_unexpected)?,
    )
}

pub fn get(config: &Config) -> Result<u64, CoreError> {
    let maybe_value: Option<Vec<u8>> = local_storage::read(config, LAST_UPDATED, LAST_UPDATED)?;
    match maybe_value {
        None => Ok(0),
        Some(value) => Ok(bincode::deserialize(value.as_ref()).map_err(core_err_unexpected)?),
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::last_updated_repo;

    #[test]
    fn get() {
        let config = &temp_config();

        let result = last_updated_repo::get(config).unwrap();

        assert_eq!(result, 0);
    }

    #[test]
    fn set_maybe_get() {
        let config = &temp_config();

        last_updated_repo::set(config, 42069).unwrap();
        let result = last_updated_repo::get(config).unwrap();

        assert_eq!(result, 42069);
    }
}
