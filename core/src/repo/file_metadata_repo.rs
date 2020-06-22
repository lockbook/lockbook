use crate::error_enum;
use crate::model::client_file_metadata::ClientFileMetadata;
use crate::repo::file_metadata_repo::FindingParentsFailed::AncestorMissing;
use sled::Db;
use std::collections::HashMap;
use uuid::Uuid;

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

#[derive(Debug)]
pub enum FindingParentsFailed {
    AncestorMissing,
    DbError(DbError),
}

pub trait FileMetadataRepo {
    fn insert(db: &Db, file: &ClientFileMetadata) -> Result<(), DbError>;
    fn get(db: &Db, id: Uuid) -> Result<ClientFileMetadata, Error>;
    fn maybe_get(db: &Db, id: Uuid) -> Result<Option<ClientFileMetadata>, DbError>;
    fn find_by_name(db: &Db, name: &str) -> Result<Option<ClientFileMetadata>, DbError>;
    fn get_with_all_parents(
        db: &Db,
        id: Uuid,
    ) -> Result<HashMap<Uuid, ClientFileMetadata>, FindingParentsFailed>;
    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, DbError>;
    fn get_all_dirty(db: &Db) -> Result<Vec<ClientFileMetadata>, Error>;
    fn actually_delete(db: &Db, id: Uuid) -> Result<u64, Error>;
    fn set_last_updated(db: &Db, last_updated: u64) -> Result<(), Error>;
    fn get_last_updated(db: &Db) -> Result<u64, Error>;
}

pub struct FileMetadataRepoImpl;

static FILE_METADATA: &[u8; 13] = b"file_metadata";
static LAST_UPDATED: &[u8; 12] = b"last_updated";

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert(db: &Db, file: &ClientFileMetadata) -> Result<(), DbError> {
        let tree = db.open_tree(FILE_METADATA)?;
        tree.insert(&file.id.as_bytes(), serde_json::to_vec(&file)?)?;
        Ok(())
    }

    fn get(db: &Db, id: Uuid) -> Result<ClientFileMetadata, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        let maybe_value = tree.get(id.as_bytes())?;
        let value = maybe_value.ok_or(())?;
        let file_metadata: ClientFileMetadata = serde_json::from_slice(value.as_ref())?;

        Ok(file_metadata)
    }

    fn maybe_get(db: &Db, id: Uuid) -> Result<Option<ClientFileMetadata>, DbError> {
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

    fn find_by_name(db: &Db, name: &str) -> Result<Option<ClientFileMetadata>, DbError> {
        let all = FileMetadataRepoImpl::get_all(&db)?;
        for file in all {
            if file.name == name {
                return Ok(Some(file));
            }
        }
        Ok(None)
    }

    fn get_with_all_parents(
        db: &Db,
        id: Uuid,
    ) -> Result<HashMap<Uuid, ClientFileMetadata>, FindingParentsFailed> {
        let mut parents = HashMap::new();
        let mut current_id = id;

        loop {
            match Self::maybe_get(&db, current_id).map_err(FindingParentsFailed::DbError)? {
                Some(found) => {
                    parents.insert(current_id, found.clone());
                    if found.id == found.parent_id {
                        return Ok(parents);
                    } else {
                        current_id = found.parent_id;
                        continue;
                    }
                }
                None => return Err(AncestorMissing),
            }
        }
    }

    fn get_all(db: &Db) -> Result<Vec<ClientFileMetadata>, DbError> {
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
            .filter(|file| file.new || file.document_edited || file.metadata_changed)
            .collect::<Vec<ClientFileMetadata>>())
    }

    fn actually_delete(db: &Db, id: Uuid) -> Result<u64, Error> {
        let tree = db.open_tree(FILE_METADATA)?;
        tree.remove(id.as_bytes())?;
        Ok(1)
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
}

#[cfg(test)]
mod unit_tests {
    use crate::model::client_file_metadata::{ClientFileMetadata, FileType};
    use crate::model::crypto::{EncryptedValueWithNonce, FolderAccessInfo};
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
    use uuid::Uuid;

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn insert_file_metadata() {
        let test_file_metadata = ClientFileMetadata {
            file_type: FileType::Document,
            id: Uuid::new_v4(),
            name: "".to_string(),
            parent_id: Default::default(),
            content_version: 0,
            metadata_version: 0,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: Default::default(),
                access_key: EncryptedValueWithNonce {
                    garbage: "".to_string(),
                    nonce: "".to_string(),
                },
            },
            new: false,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileMetadataRepoImpl::insert(&db, &test_file_metadata).unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get(&db, test_file_metadata.id).unwrap();
        assert_eq!(test_file_metadata.name, db_file_metadata.name);
        assert_eq!(test_file_metadata.parent_id, db_file_metadata.parent_id);
    }

    #[test]
    fn update_file_metadata() {
        let id = Uuid::new_v4();
        let parent = Uuid::new_v4();
        let test_meta = ClientFileMetadata {
            file_type: FileType::Document,
            id: id,
            name: "".to_string(),
            parent_id: parent,
            content_version: 0,
            metadata_version: 0,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: Uuid::new_v4(),
                access_key: EncryptedValueWithNonce { garbage: "".to_string(), nonce: "".to_string() }
            },
            new: false,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
        };
        let test_meta_updated = ClientFileMetadata {
            file_type: FileType::Document,
            id: id,
            name: "".to_string(),
            parent_id: parent,
            content_version: 1000,
            metadata_version: 1000,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: Uuid::new_v4(),
                access_key: EncryptedValueWithNonce { garbage: "".to_string(), nonce: "".to_string() }
            },
            new: false,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileMetadataRepoImpl::insert(&db, &test_meta).unwrap();
        assert_eq!(
            test_meta.content_version,
            FileMetadataRepoImpl::get(&db, test_meta.id)
                .unwrap()
                .content_version
        );
        FileMetadataRepoImpl::insert(&db, &test_meta_updated).unwrap();
        assert_eq!(
            test_meta_updated.content_version,
            FileMetadataRepoImpl::get(&db, test_meta_updated.id)
                .unwrap()
                .content_version
        );
    }
}
