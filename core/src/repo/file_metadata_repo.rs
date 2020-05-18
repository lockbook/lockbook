use std::option::NoneError;

use serde_json;
use sled;

use crate::error_enum;
use crate::model::client_file_metadata::ClientFileMetadata;
use sled::Db;

error_enum! {
    enum DbError {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
    }
}

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        FileRowMissing(NoneError),
    }
}

pub trait FileMetadataRepo {
    fn insert_new_file(db: &Db, name: &String, path: &String) -> Result<ClientFileMetadata, Error>;
    fn update(db: &Db, file_metadata: &ClientFileMetadata) -> Result<ClientFileMetadata, Error>;
    fn maybe_get(db: &Db, id: &String) -> Result<Option<ClientFileMetadata>, DbError>;
    fn get(db: &Db, id: &String) -> Result<ClientFileMetadata, Error>;
    fn set_last_updated(db: &Db, last_updated: &u64) -> Result<(), Error>;
    fn get_last_updated(db: &Db) -> Result<u64, Error>;
    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, Error>;
    fn get_all_dirty(db: &Db) -> Result<Vec<ClientFileMetadata>, Error>;
    fn delete(db: &Db, id: &String) -> Result<u64, Error>;
}

pub struct FileMetadataRepoImpl;

static FILE_METADATA: &[u8; 13] = b"file_metadata";
static LAST_UPDATED: &[u8; 12] = b"last_updated";

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert_new_file(db: &Db, name: &String, path: &String) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        let meta = ClientFileMetadata::new_file(&name, &path);
        tree.insert(meta.file_id.as_bytes(), serde_json::to_vec(&meta)?)?;
        Ok(meta)
    }

    fn update(db: &Db, file_metadata: &ClientFileMetadata) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        tree.insert(
            file_metadata.file_id.as_bytes(),
            serde_json::to_vec(&file_metadata)?,
        )?;
        Ok(file_metadata.clone())
    }

    fn maybe_get(db: &Db, id: &String) -> Result<Option<ClientFileMetadata>, DbError> {
        let tree = db.open_tree(FILE_METADATA)?;
        let maybe_value = tree.get(id.as_bytes())?;
        match maybe_value {
            None => Ok(None),
            Some(value) => {
                let file_metadata: ClientFileMetadata = serde_json::from_slice(value.as_ref())?;
                Ok(Some(file_metadata))
            }
        }
    }

    fn get(db: &Db, id: &String) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        let maybe_value = tree.get(id.as_bytes())?;
        let value = maybe_value?;
        let file_metadata: ClientFileMetadata = serde_json::from_slice(value.as_ref())?;

        Ok(file_metadata)
    }

    fn set_last_updated(db: &Db, last_updated: &u64) -> Result<(), Error> {
        let tree = db.open_tree(LAST_UPDATED)?;
        tree.insert(LAST_UPDATED, serde_json::to_vec(&last_updated)?)?;
        Ok(())
    }

    fn get_last_updated(db: &Db) -> Result<u64, Error> {
        let tree = db.open_tree(LAST_UPDATED)?;
        let maybe_value = tree.get(LAST_UPDATED)?;
        match maybe_value {
            None => Ok(0),
            Some(value) => Ok(serde_json::from_slice(value.as_ref())?),
        }
    }

    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        let value = tree
            .iter()
            .map(|s| {
                let meta: ClientFileMetadata =
                    serde_json::from_slice(s.unwrap().1.as_ref()).unwrap();
                meta
            })
            .collect::<Vec<ClientFileMetadata>>();

        Ok(value)
    }

    fn get_all_dirty(db: &Db) -> Result<Vec<ClientFileMetadata>, Error> {
        // TODO test
        let tree = db.open_tree(b"file_metadata")?;
        let all_files = tree
            .iter()
            .map(|s| {
                let meta: ClientFileMetadata =
                    serde_json::from_slice(s.unwrap().1.as_ref()).unwrap();
                meta
            })
            .collect::<Vec<ClientFileMetadata>>();
        Ok(all_files
            .into_iter()
            .filter(|file| {
                file.new_file || file.content_edited_locally || file.metadata_edited_locally
            })
            .collect::<Vec<ClientFileMetadata>>())
    }

    fn delete(db: &Db, id: &String) -> Result<u64, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        tree.remove(id.as_bytes())?;
        Ok(1)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::client_file_metadata::ClientFileMetadata;
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn insert_file_metadata() {
        let test_file_metadata =
            ClientFileMetadata::new_file(&("test_file".to_string()), &("test_file".to_string()));

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        let meta_res = FileMetadataRepoImpl::insert_new_file(
            &db,
            &test_file_metadata.file_name,
            &test_file_metadata.file_path,
        )
        .unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get(&db, &meta_res.file_id).unwrap();
        assert_eq!(test_file_metadata.file_name, db_file_metadata.file_name);
        assert_eq!(test_file_metadata.file_path, db_file_metadata.file_path);
    }

    #[test]
    fn update_file_metadata() {
        let test_meta = ClientFileMetadata {
            file_id: "".to_string(),
            file_name: "".to_string(),
            file_path: "".to_string(),
            file_content_version: 0,
            file_metadata_version: 0,
            new_file: false,
            content_edited_locally: false,
            metadata_edited_locally: false,
            deleted_locally: false,
        };
        let test_meta_updated = ClientFileMetadata {
            file_id: "".to_string(),
            file_name: "".to_string(),
            file_path: "".to_string(),
            file_content_version: 1000,
            file_metadata_version: 1000,
            new_file: false,
            content_edited_locally: false,
            metadata_edited_locally: false,
            deleted_locally: false,
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        let meta_res =
            FileMetadataRepoImpl::insert_new_file(&db, &test_meta.file_name, &test_meta.file_path)
                .unwrap();
        assert_eq!(
            test_meta.file_content_version,
            FileMetadataRepoImpl::get(&db, &meta_res.file_id)
                .unwrap()
                .file_content_version
        );
        let meta_upd_res = FileMetadataRepoImpl::update(&db, &test_meta_updated).unwrap();
        assert_eq!(
            test_meta_updated.file_content_version,
            FileMetadataRepoImpl::get(&db, &meta_upd_res.file_id)
                .unwrap()
                .file_content_version
        );
    }
}
