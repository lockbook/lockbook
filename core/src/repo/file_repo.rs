use std::option::NoneError;

use crate::error_enum;
use crate::model::file::File;
use crate::model::file_metadata::FileMetadata;
use serde_json;
use sled;
use sled::Db;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        FileRowMissing(NoneError)
    }
}

pub trait FileRepo {
    fn update(db: &Db, file: &File) -> Result<(), Error>;
    fn get(db: &Db, id: &String) -> Result<File, Error>;
}

pub struct FileRepoImpl;

impl FileRepo for FileRepoImpl {
    fn update(db: &Db, file: &File) -> Result<(), Error> {
        let tree = db.open_tree(b"files")?;
        tree.insert(file.id.as_bytes(), serde_json::to_vec(file)?)?;
        Ok(())
    }

    fn get(db: &Db, id: &String) -> Result<File, Error> {
        let tree = db.open_tree(b"files")?;
        let maybe_value = tree.get(id.as_bytes())?;
        let value = maybe_value?;
        let file: File = serde_json::from_slice(value.as_ref())?;

        Ok(file)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::file::File;
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_repo::{FileRepo, FileRepoImpl};

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn update_file() {
        let test_file = File {
            id: "a".to_string(),
            content: "some stuff".to_string(),
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileRepoImpl::update(&db, &test_file).unwrap();

        let file = FileRepoImpl::get(&db, &"a".to_string()).unwrap();
        assert_eq!(file.content, "some stuff");

        FileRepoImpl::update(
            &db,
            &File {
                id: file.id,
                content: "updated stuff".to_string(),
            },
        )
        .unwrap();

        let file_updated = FileRepoImpl::get(&db, &"a".to_string()).unwrap();

        assert_eq!(file_updated.content, "updated stuff");
    }
}
