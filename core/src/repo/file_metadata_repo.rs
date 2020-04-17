use std::option::NoneError;

use serde_json;
use sled;

use crate::error_enum;
use crate::model::file_metadata::{FileMetadata, Status};
use sled::{Db, IVec};
use std::time::SystemTime;
use uuid::Uuid;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        SystemTimeError(std::time::SystemTimeError),
        FileRowMissing(NoneError),
    }
}

pub trait FileMetadataRepo {
    fn insert(db: &Db, name: &String, path: &String) -> Result<FileMetadata, Error>;
    fn update(db: &Db, file_metadata: &FileMetadata) -> Result<FileMetadata, Error>;
    fn get(db: &Db, id: &String) -> Result<FileMetadata, Error>;
    fn last_updated(db: &Db) -> Result<u64, Error>;
    fn get_all(db: &Db) -> Result<Vec<FileMetadata>, Error>;
    fn delete(db: &Db, id: &String) -> Result<u64, Error>;
}

pub struct FileMetadataRepoImpl;

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert(db: &Db, name: &String, path: &String) -> Result<FileMetadata, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        let version = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_micros() as u64;
        let meta = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: name.clone(),
            path: path.clone(),
            updated_at: version.clone(),
            version: version.clone(),
            status: Status::New,
        };
        tree.insert(meta.id.as_bytes(), serde_json::to_vec(&meta)?)?;
        Ok(meta)
    }

    fn update(db: &Db, file_metadata: &FileMetadata) -> Result<FileMetadata, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        let meta = FileMetadata {
            id: file_metadata.id.clone(),
            name: file_metadata.name.clone(),
            path: file_metadata.path.clone(),
            updated_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_micros() as u64,
            version: file_metadata.version,
            status: file_metadata.status.clone(),
        };
        tree.insert(meta.id.as_bytes(), serde_json::to_vec(&meta)?)?;
        Ok(meta)
    }

    fn get(db: &Db, id: &String) -> Result<FileMetadata, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        let maybe_value = tree.get(id.as_bytes())?;
        let value = maybe_value?;
        let file_metadata: FileMetadata = serde_json::from_slice(value.as_ref())?;

        Ok(file_metadata)
    }

    fn last_updated(db: &Db) -> Result<u64, Error> {
        Ok(Self::get_all(db)?.iter().fold(0, |max, meta| {
            // if meta.status != Status::Local {
            max.max(meta.updated_at)
            // } else {
            //     max
            // }
        }))
    }

    fn get_all(db: &Db) -> Result<Vec<FileMetadata>, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        let value = tree
            .iter()
            .map(|s| {
                let meta: FileMetadata = serde_json::from_slice(s.unwrap().1.as_ref()).unwrap();
                meta
            })
            .collect::<Vec<FileMetadata>>();

        Ok(value)
    }

    fn delete(db: &Db, id: &String) -> Result<u64, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        tree.remove(id.as_bytes())?;
        Ok(1)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::file_metadata::{FileMetadata, Status};
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
    use uuid::Uuid;

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn insert_file_metadata() {
        let test_file_metadata = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "test_file".to_string(),
            path: "a/b/c".to_string(),
            updated_at: 1234,
            version: 1234,
            status: Status::Local,
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileMetadataRepoImpl::insert(&db, &test_file_metadata.name, &test_file_metadata.path)
            .unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get(&db, &test_file_metadata.id).unwrap();
        assert_eq!(test_file_metadata, db_file_metadata);
    }

    #[test]
    fn update_file_metadata() {
        let test_meta = FileMetadata {
            id: "test".to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 0,
            version: 0,
            status: Status::Local,
        };
        let test_meta_updated = FileMetadata {
            id: "test".to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 1000,
            version: 1000,
            status: Status::Local,
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileMetadataRepoImpl::insert(&db, &test_meta.name, &test_meta.path).unwrap();
        assert_eq!(
            test_meta,
            FileMetadataRepoImpl::get(&db, &test_meta.id).unwrap()
        );
        FileMetadataRepoImpl::update(&db, &test_meta_updated).unwrap();
        assert_eq!(
            test_meta_updated,
            FileMetadataRepoImpl::get(&db, &test_meta.id).unwrap()
        );
    }

    #[test]
    fn dump_repo_get_max() {
        let test_meta1 = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 0,
            version: 0,
            status: Status::Local,
        };
        let test_meta2 = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 100,
            version: 100,
            status: Status::Local,
        };
        let test_meta3 = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 9000,
            version: 9000,
            status: Status::Local,
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileMetadataRepoImpl::insert(&db, &test_meta1.name, &test_meta1.path).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_meta2.name, &test_meta2.path).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_meta3.name, &test_meta3.path).unwrap();

        let all_files = FileMetadataRepoImpl::get_all(&db).unwrap();
        println!("{:?}", all_files);
        assert_eq!(all_files.len(), 3);

        let updated_max = FileMetadataRepoImpl::last_updated(&db).unwrap();
        assert_eq!(updated_max, 9000);
    }
}
