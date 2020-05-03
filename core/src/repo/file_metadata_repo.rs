use std::option::NoneError;

use serde_json;
use sled;

use crate::error_enum;
use crate::model::client_file_metadata::ClientFileMetadata;
use sled::Db;

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
    fn get(db: &Db, id: &String) -> Result<ClientFileMetadata, Error>;
    fn last_updated(db: &Db) -> Result<u64, Error>;
    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, Error>;
    fn delete(db: &Db, id: &String) -> Result<u64, Error>;
}

pub struct FileMetadataRepoImpl;

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert_new_file(db: &Db, name: &String, _path: &String) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        let meta = ClientFileMetadata::new_file(&name);
        tree.insert(meta.file_id.as_bytes(), serde_json::to_vec(&meta)?)?;
        Ok(meta)
    }

    fn update(db: &Db, file_metadata: &ClientFileMetadata) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        tree.insert(file_metadata.file_id.as_bytes(), serde_json::to_vec(&file_metadata)?)?;
        Ok(file_metadata.clone())
    }

    fn get(db: &Db, id: &String) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        let maybe_value = tree.get(id.as_bytes())?;
        let value = maybe_value?;
        let file_metadata: ClientFileMetadata = serde_json::from_slice(value.as_ref())?;

        Ok(file_metadata)
    }

    fn last_updated(db: &Db) -> Result<u64, Error> {
        Ok(Self::get_all(db)?
            .iter()
            .fold(0, |max, meta| max.max(meta.file_content_version)))
    }

    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, Error> {
        let tree = db.open_tree(b"file_metadata")?;
        let value = tree
            .iter()
            .map(|s| {
                let meta: ClientFileMetadata = serde_json::from_slice(s.unwrap().1.as_ref()).unwrap();
                meta
            })
            .collect::<Vec<ClientFileMetadata>>();

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
    use crate::model::client_file_metadata::ClientFileMetadata;
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
    use uuid::Uuid;

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn insert_file_metadata() {
        let test_file_metadata = ClientFileMetadata::new_file(&("test_file".to_string()));

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        let meta_res =
            FileMetadataRepoImpl::insert_new_file(&db, &test_file_metadata.file_name, &test_file_metadata.file_path)
                .unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get(&db, &meta_res.file_id).unwrap();
        assert_eq!(test_file_metadata, db_file_metadata);
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
            deleted_locally: false
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
            deleted_locally: false
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        let meta_res = FileMetadataRepoImpl::insert_new_file(&db, &test_meta.file_name, &test_meta.file_path).unwrap();
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

    #[test]
    fn dump_repo_get_max() {
        let test_meta1 = ClientFileMetadata {
            file_id: Uuid::new_v4().to_string(),
            file_name: "".to_string(),
            file_path: "".to_string(),
            file_content_version: 0,
            file_metadata_version: 0,
            new_file: false,
            content_edited_locally: false,
            metadata_edited_locally: false,
            deleted_locally: false
        };
        let test_meta2 = ClientFileMetadata {
            file_id: Uuid::new_v4().to_string(),
            file_name: "".to_string(),
            file_path: "".to_string(),
            file_content_version: 100,
            file_metadata_version: 100,
            new_file: false,
            content_edited_locally: false,
            metadata_edited_locally: false,
            deleted_locally: false
        };
        let test_meta3 = ClientFileMetadata {
            file_id: Uuid::new_v4().to_string(),
            file_name: "".to_string(),
            file_path: "".to_string(),
            file_content_version: 9000,
            file_metadata_version: 9000,
            new_file: false,
            content_edited_locally: false,
            metadata_edited_locally: false,
            deleted_locally: false
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileMetadataRepoImpl::insert_new_file(&db, &test_meta1.file_name, &test_meta1.file_path).unwrap();
        FileMetadataRepoImpl::insert_new_file(&db, &test_meta2.file_name, &test_meta2.file_path).unwrap();
        FileMetadataRepoImpl::insert_new_file(&db, &test_meta3.file_name, &test_meta3.file_path).unwrap();

        let all_files = FileMetadataRepoImpl::get_all(&db).unwrap();
        assert_eq!(all_files.len(), 3);

        let updated_max = FileMetadataRepoImpl::last_updated(&db).unwrap();
        assert_eq!(updated_max, 0);
    }
}
