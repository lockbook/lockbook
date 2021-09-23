use crate::core_err_unexpected;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;

static DB_VERSION: &str = "DB_VERSION";

pub fn set(config: &Config, version: &str) -> Result<(), CoreError> {
    local_storage::write(
        config,
        DB_VERSION,
        DB_VERSION.as_bytes(),
        serde_json::to_vec(version).map_err(core_err_unexpected)?,
    )
}

pub fn maybe_get(config: &Config) -> Result<Option<String>, CoreError> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, DB_VERSION, DB_VERSION.as_bytes())?;
    match maybe_value {
        None => Ok(None),
        Some(file) => {
            let version: String =
                serde_json::from_slice(file.as_ref()).map_err(core_err_unexpected)?;

            Ok(Some(version))
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::db_version_repo;

    #[test]
    fn maybe_get() {
        let config = &temp_config();

        let result = db_version_repo::maybe_get(config).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn set_maybe_get() {
        let config = &temp_config();

        db_version_repo::set(config, "version").unwrap();
        let result = db_version_repo::maybe_get(config).unwrap();

        assert_eq!(result, Some(String::from("version")));
    }
}
