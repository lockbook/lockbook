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
        FileRowMissing(()),
    }
}

pub trait FileMetadataRepo {
    fn insert_new_file(db: &Db, name: &String, path: &String) -> Result<ClientFileMetadata, Error>;
    fn update(db: &Db, file_metadata: &ClientFileMetadata) -> Result<ClientFileMetadata, Error>;
    fn maybe_get(db: &Db, id: &String) -> Result<Option<ClientFileMetadata>, DbError>;
    fn get(db: &Db, id: &String) -> Result<ClientFileMetadata, Error>;
    fn find_by_name(db: &Db, name: &String) -> Result<Option<ClientFileMetadata>, DbError>;
    fn set_last_updated(db: &Db, last_updated: u64) -> Result<(), Error>;
    fn get_last_updated(db: &Db) -> Result<u64, Error>;
    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, DbError>;
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
        tree.insert(meta.id.as_bytes(), serde_json::to_vec(&meta)?)?;
        Ok(meta)
    }

    fn update(db: &Db, file_metadata: &ClientFileMetadata) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        tree.insert(
            file_metadata.id.as_bytes(),
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
        let value = maybe_value.ok_or(())?;
        let file_metadata: ClientFileMetadata = serde_json::from_slice(value.as_ref())?;

        Ok(file_metadata)
    }

    fn find_by_name(db: &Db, name: &String) -> Result<Option<ClientFileMetadata>, DbError> {
        let all = FileMetadataRepoImpl::get_all(&db)?;
        for file in all {
            if &file.name == name {
                return Ok(Some(file));
            }
        }
        Ok(None)
    }

    fn set_last_updated(db: &Db, last_updated: u64) -> Result<(), Error> {
        debug!("Setting last updated to: {}", last_updated);
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

    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, DbError> {
        debug!("Test");
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
                file.new || file.document_edited || file.metadata_changed
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
            &test_file_metadata.name,
            &test_file_metadata.parent_id,
        )
        .unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get(&db, &meta_res.id).unwrap();
        assert_eq!(test_file_metadata.name, db_file_metadata.name);
        assert_eq!(test_file_metadata.parent_id, db_file_metadata.parent_id);
    }

    #[test]
    fn update_file_metadata() {
        let test_meta = ClientFileMetadata {
            id: "".to_string(),
            name: "".to_string(),
            parent_id: "".to_string(),
            content_version: 0,
            metadata_version: 0,
            new: false,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
        };
        let test_meta_updated = ClientFileMetadata {
            id: "".to_string(),
            name: "".to_string(),
            parent_id: "".to_string(),
            content_version: 1000,
            metadata_version: 1000,
            new: false,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        let meta_res =
            FileMetadataRepoImpl::insert_new_file(&db, &test_meta.name, &test_meta.parent_id)
                .unwrap();
        assert_eq!(
            test_meta.content_version,
            FileMetadataRepoImpl::get(&db, &meta_res.id)
                .unwrap()
                .content_version
        );
        let meta_upd_res = FileMetadataRepoImpl::update(&db, &test_meta_updated).unwrap();
        assert_eq!(
            test_meta_updated.content_version,
            FileMetadataRepoImpl::get(&db, &meta_upd_res.id)
                .unwrap()
                .content_version
        );
    }
}
