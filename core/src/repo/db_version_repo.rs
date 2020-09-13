use sled::Db;

#[derive(Debug)]
pub enum Error {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
}

pub trait DbVersionRepo {
    fn set(db: &Db, version: &str) -> Result<(), Error>;
    fn get(db: &Db) -> Result<Option<String>, Error>;
}

pub struct DbVersionRepoImpl;

static DB_VERSION: &str = "DB_VERSION";

impl DbVersionRepo for DbVersionRepoImpl {
    fn set(db: &Db, version: &str) -> Result<(), Error> {
        let tree = db.open_tree(DB_VERSION).map_err(Error::SledError)?;
        tree.insert(
            DB_VERSION.as_bytes(),
            serde_json::to_vec(version).map_err(Error::SerdeError)?,
        )
        .map_err(Error::SledError)?;
        Ok(())
    }

    fn get(db: &Db) -> Result<Option<String>, Error> {
        let tree = db.open_tree(DB_VERSION).map_err(Error::SledError)?;
        match tree.get(DB_VERSION.as_bytes()).map_err(Error::SledError)? {
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
    use crate::model::state::dummy_config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::db_version_repo::{DbVersionRepo, DbVersionRepoImpl};

    #[test]
    fn db_version_sanity_check() {
        let db = TempBackedDB::connect_to_db(&dummy_config()).unwrap();

        assert!(DbVersionRepoImpl::get(&db).unwrap().is_none());
        DbVersionRepoImpl::set(&db, "version 1").unwrap();
        assert_eq!(DbVersionRepoImpl::get(&db).unwrap().unwrap(), "version 1");
        DbVersionRepoImpl::set(&db, "version 2").unwrap();
        assert_eq!(DbVersionRepoImpl::get(&db).unwrap().unwrap(), "version 2");
    }
}
