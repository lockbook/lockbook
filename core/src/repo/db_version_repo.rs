use crate::storage::db_provider::Backend;

#[derive(Debug)]
pub enum Error<MyBackend: Backend> {
    BackendError(MyBackend::Error),
    SerdeError(serde_json::Error),
}

pub trait DbVersionRepo<MyBackend: Backend> {
    fn set(backend: &MyBackend::Db, version: &str) -> Result<(), Error<MyBackend>>;
    fn get(backend: &MyBackend::Db) -> Result<Option<String>, Error<MyBackend>>;
}

pub struct DbVersionRepoImpl<MyBackend: Backend> {
    _backend: MyBackend,
}

static DB_VERSION: &str = "DB_VERSION";

impl<MyBackend: Backend> DbVersionRepo<MyBackend> for DbVersionRepoImpl<MyBackend> {
    fn set(backend: &MyBackend::Db, version: &str) -> Result<(), Error<MyBackend>> {
        MyBackend::write(
            backend,
            DB_VERSION,
            DB_VERSION.as_bytes(),
            serde_json::to_vec(version).map_err(Error::SerdeError)?,
        )
        .map_err(Error::BackendError)
    }

    fn get(backend: &MyBackend::Db) -> Result<Option<String>, Error<MyBackend>> {
        let maybe_value: Option<Vec<u8>> =
            MyBackend::read(backend, DB_VERSION, DB_VERSION.as_bytes())
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
    use crate::repo::db_version_repo::DbVersionRepo;
    use crate::storage::db_provider::Backend;
    use crate::{
        model::state::temp_config, storage::db_provider::FileBackend, DefaultDbVersionRepo,
    };

    #[test]
    fn db_version_sanity_check() {
        let cfg = &temp_config();
        let backend = FileBackend::connect_to_db(&cfg).unwrap();

        assert!(DefaultDbVersionRepo::get(&backend).unwrap().is_none());
        DefaultDbVersionRepo::set(&backend, "version 1").unwrap();
        assert_eq!(
            DefaultDbVersionRepo::get(&backend).unwrap().unwrap(),
            "version 1"
        );
        DefaultDbVersionRepo::set(&backend, "version 2").unwrap();
        assert_eq!(
            DefaultDbVersionRepo::get(&backend).unwrap().unwrap(),
            "version 2"
        );
    }
}
