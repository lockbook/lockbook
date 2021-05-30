use crate::model::state::Config;
use crate::repo::local_storage;

#[derive(Debug)]
pub enum Error {
    BackendError(std::io::Error),
    SerdeError(serde_json::Error),
}

static DB_VERSION: &str = "DB_VERSION";

pub fn set(config: &Config, version: &str) -> Result<(), Error> {
    local_storage::write(
        config,
        DB_VERSION,
        DB_VERSION.as_bytes(),
        serde_json::to_vec(version).map_err(Error::SerdeError)?,
    )
    .map_err(Error::BackendError)
}

pub fn get(config: &Config) -> Result<Option<String>, Error> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, DB_VERSION, DB_VERSION.as_bytes())
            .map_err(Error::BackendError)?;
    match maybe_value {
        None => Ok(None),
        Some(file) => {
            let version: String =
                serde_json::from_slice(file.as_ref()).map_err(Error::SerdeError)?;

            Ok(Some(version))
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::db_version_repo;

    #[test]
    fn db_version_sanity_check() {
        let config = temp_config();

        assert!(db_version_repo::get(&config).unwrap().is_none());
        db_version_repo::set(&config, "version 1").unwrap();
        assert_eq!(db_version_repo::get(&config).unwrap().unwrap(), "version 1");
        db_version_repo::set(&config, "version 2").unwrap();
        assert_eq!(db_version_repo::get(&config).unwrap().unwrap(), "version 2");
    }
}
