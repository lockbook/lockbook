use crate::storage::db_provider;
use crate::storage::db_provider::Backend;

#[derive(Debug)]
pub enum Error {
    BackendError(db_provider::BackendError),
    SerdeError(serde_json::Error),
}

pub trait DbVersionRepo {
    fn set(backend: &Backend, version: &str) -> Result<(), Error>;
    fn get(backend: &Backend) -> Result<Option<String>, Error>;
}

pub struct DbVersionRepoImpl;

static DB_VERSION: &str = "DB_VERSION";

impl DbVersionRepo for DbVersionRepoImpl {
    fn set(backend: &Backend, version: &str) -> Result<(), Error> {
        backend
            .write(
                DB_VERSION,
                DB_VERSION.as_bytes(),
                serde_json::to_vec(version).map_err(Error::SerdeError)?,
            )
            .map_err(Error::BackendError)
    }

    fn get(backend: &Backend) -> Result<Option<String>, Error> {
        let maybe_value: Option<Vec<u8>> = backend
            .read(DB_VERSION, DB_VERSION.as_bytes())
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
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::db_version_repo::{DbVersionRepo, DbVersionRepoImpl};
    use crate::storage::db_provider::to_backend;

    #[test]
    fn db_version_sanity_check() {
        let cfg = &temp_config();
        let backend = &to_backend(cfg);

        assert!(DbVersionRepoImpl::get(backend).unwrap().is_none());
        DbVersionRepoImpl::set(backend, "version 1").unwrap();
        assert_eq!(
            DbVersionRepoImpl::get(backend).unwrap().unwrap(),
            "version 1"
        );
        DbVersionRepoImpl::set(backend, "version 2").unwrap();
        assert_eq!(
            DbVersionRepoImpl::get(backend).unwrap().unwrap(),
            "version 2"
        );
    }
}
