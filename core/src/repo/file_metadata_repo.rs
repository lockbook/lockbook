use std::collections::HashMap;

use sled::Db;
use uuid::Uuid;

use crate::model::file_metadata::FileType::{Document, Folder};
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::repo::file_metadata_repo::FindingParentsFailed::AncestorMissing;

#[derive(Debug)]
pub enum DbError {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
}

#[derive(Debug)]
pub enum Error {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
    FileRowMissing(()),
}

#[derive(Debug)]
pub enum FindingParentsFailed {
    AncestorMissing,
    DbError(DbError),
}

pub enum Filter {
    DocumentsOnly,
    FoldersOnly,
    LeafNodesOnly,
}

pub trait FileMetadataRepo {
    fn insert(db: &Db, file: &FileMetadata) -> Result<(), DbError>;
    fn get_root(db: &Db) -> Result<Option<FileMetadata>, DbError>;
    fn get(db: &Db, id: Uuid) -> Result<FileMetadata, Error>;
    fn maybe_get(db: &Db, id: Uuid) -> Result<Option<FileMetadata>, DbError>;
    fn get_by_path(db: &Db, path: &str) -> Result<Option<FileMetadata>, DbError>;
    fn get_with_all_parents(
        db: &Db,
        id: Uuid,
    ) -> Result<HashMap<Uuid, FileMetadata>, FindingParentsFailed>;
    fn get_all(db: &Db) -> Result<Vec<FileMetadata>, DbError>;
    fn get_all_paths(db: &Db, filter: Option<Filter>) -> Result<Vec<String>, FindingParentsFailed>;
    fn actually_delete(db: &Db, id: Uuid) -> Result<u64, Error>;
    fn get_children(db: &Db, id: Uuid) -> Result<Vec<FileMetadata>, DbError>;
    fn set_last_updated(db: &Db, last_updated: u64) -> Result<(), Error>;
    fn get_last_updated(db: &Db) -> Result<u64, Error>;
}

pub struct FileMetadataRepoImpl;

static FILE_METADATA: &[u8; 13] = b"file_metadata";
static ROOT: &[u8; 4] = b"ROOT";
static LAST_UPDATED: &[u8; 12] = b"last_updated";

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert(db: &Db, file: &FileMetadata) -> Result<(), DbError> {
        let tree = db.open_tree(FILE_METADATA).map_err(DbError::SledError)?;
        tree.insert(
            &file.id.as_bytes(),
            serde_json::to_vec(&file).map_err(DbError::SerdeError)?,
        )
        .map_err(DbError::SledError)?;
        if file.id == file.parent {
            let root = db.open_tree(ROOT).map_err(DbError::SledError)?;
            debug!("saving root folder: {:?}", &file.id);
            root.insert(
                ROOT,
                serde_json::to_vec(&file.id).map_err(DbError::SerdeError)?,
            )
            .map_err(DbError::SledError)?;
        }
        Ok(())
    }

    fn get_root(db: &Db) -> Result<Option<FileMetadata>, DbError> {
        let tree = db.open_tree(ROOT).map_err(DbError::SledError)?;
        let maybe_value = tree.get(ROOT).map_err(DbError::SledError)?;
        match maybe_value {
            None => Ok(None),
            Some(value) => {
                let id: Uuid =
                    serde_json::from_slice(value.as_ref()).map_err(DbError::SerdeError)?;
                Self::maybe_get(&db, id)
            }
        }
    }

    fn get(db: &Db, id: Uuid) -> Result<FileMetadata, Error> {
        let tree = db.open_tree(FILE_METADATA).map_err(Error::SledError)?;
        let maybe_value = tree.get(id.as_bytes()).map_err(Error::SledError)?;
        let value = maybe_value.ok_or(()).map_err(Error::FileRowMissing)?;
        let file_metadata: FileMetadata =
            serde_json::from_slice(value.as_ref()).map_err(Error::SerdeError)?;

        Ok(file_metadata)
    }

    fn maybe_get(db: &Db, id: Uuid) -> Result<Option<FileMetadata>, DbError> {
        let tree = db.open_tree(FILE_METADATA).map_err(DbError::SledError)?;
        let maybe_value = tree.get(id.as_bytes()).map_err(DbError::SledError)?;
        match maybe_value {
            None => Ok(None),
            Some(value) => {
                let file_metadata: FileMetadata =
                    serde_json::from_slice(value.as_ref()).map_err(DbError::SerdeError)?;
                Ok(Some(file_metadata))
            }
        }
    }

    fn get_by_path(db: &Db, path: &str) -> Result<Option<FileMetadata>, DbError> {
        debug!("Path: {}", path);
        let root = match Self::get_root(&db)? {
            None => return Ok(None),
            Some(root) => root,
        };

        let mut current = root;
        let paths: Vec<&str> = path
            .split('/')
            .collect::<Vec<&str>>()
            .into_iter()
            .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
            .collect::<Vec<&str>>();

        debug!("Split length: {}", &paths.len());

        for (i, value) in paths.clone().into_iter().enumerate() {
            if value != current.name {
                return Ok(None);
            }

            if i + 1 == paths.len() {
                return Ok(Some(current));
            }

            let children = Self::get_children(&db, current.id)?;
            let mut found_child = false;
            for child in children {
                if child.name == paths[i + 1] {
                    current = child;
                    found_child = true;
                }
            }

            if !found_child {
                return Ok(None);
            }
        }

        Ok(Some(current)) // This path is never visited
    }

    fn get_with_all_parents(
        db: &Db,
        id: Uuid,
    ) -> Result<HashMap<Uuid, FileMetadata>, FindingParentsFailed> {
        let mut parents = HashMap::new();
        let mut current_id = id;
        debug!("Finding parents for: {}", current_id);

        loop {
            match Self::maybe_get(&db, current_id).map_err(FindingParentsFailed::DbError)? {
                Some(found) => {
                    debug!("Current id exists: {:?}", &found);
                    parents.insert(current_id, found.clone());
                    if found.id == found.parent {
                        return Ok(parents);
                    } else {
                        current_id = found.parent;
                        continue;
                    }
                }
                None => return Err(AncestorMissing),
            }
        }
    }

    fn get_all(db: &Db) -> Result<Vec<FileMetadata>, DbError> {
        let tree = db.open_tree(FILE_METADATA).map_err(DbError::SledError)?;
        let value = tree
            .iter()
            .map(|s| {
                let meta: FileMetadata = serde_json::from_slice(s.unwrap().1.as_ref()).unwrap();
                meta
            })
            .collect::<Vec<FileMetadata>>();

        Ok(value)
    }

    fn get_all_paths(db: &Db, filter: Option<Filter>) -> Result<Vec<String>, FindingParentsFailed> {
        let mut cache = HashMap::new();
        let mut path_cache = HashMap::new();

        // Populate metadata cache
        Self::get_all(&db)
            .map_err(FindingParentsFailed::DbError)?
            .into_iter()
            .for_each(|meta| {
                cache.insert(meta.id, meta);
            });

        for meta in cache.values() {
            saturate_path_cache(&meta, &cache, &mut path_cache)?;
        }

        let paths = match filter {
            None => path_cache.values().cloned().collect(),
            Some(filter) => match filter {
                Filter::DocumentsOnly => {
                    let mut paths = vec![];
                    for (_, meta) in cache {
                        if meta.file_type == Document {
                            if let Some(path) = path_cache.get(&meta.id) {
                                paths.push(path.to_owned())
                            }
                        }
                    }
                    paths
                }
                Filter::LeafNodesOnly => {
                    let mut paths = vec![];
                    for meta in cache.values() {
                        if is_leaf_node(meta.id, &cache) {
                            if let Some(path) = path_cache.get(&meta.id) {
                                paths.push(path.to_owned())
                            }
                        }
                    }
                    paths
                }
                Filter::FoldersOnly => {
                    let mut paths = vec![];
                    for (_, meta) in cache {
                        if meta.file_type == Folder {
                            if let Some(path) = path_cache.get(&meta.id) {
                                paths.push(path.to_owned())
                            }
                        }
                    }
                    paths
                }
            },
        };

        Ok(paths)
    }

    fn actually_delete(db: &Db, id: Uuid) -> Result<u64, Error> {
        // TODO should this be recursive?
        let tree = db.open_tree(FILE_METADATA).map_err(Error::SledError)?;
        tree.remove(id.as_bytes()).map_err(Error::SledError)?;
        Ok(1)
    }

    // TODO should this indicate something special if the parent doesn't exist?
    fn get_children(db: &Db, id: Uuid) -> Result<Vec<FileMetadata>, DbError> {
        Ok(Self::get_all(&db)?
            .into_iter()
            .filter(|file| file.parent == id && file.parent != file.id)
            .collect::<Vec<FileMetadata>>())
    }

    fn set_last_updated(db: &Db, last_updated: u64) -> Result<(), Error> {
        debug!("Setting last updated to: {}", last_updated);
        let tree = db.open_tree(LAST_UPDATED).map_err(Error::SledError)?;
        tree.insert(
            LAST_UPDATED,
            serde_json::to_vec(&last_updated).map_err(Error::SerdeError)?,
        )
        .map_err(Error::SledError)?;
        Ok(())
    }

    fn get_last_updated(db: &Db) -> Result<u64, Error> {
        let tree = db.open_tree(LAST_UPDATED).map_err(Error::SledError)?;
        let maybe_value = tree.get(LAST_UPDATED).map_err(Error::SledError)?;
        match maybe_value {
            None => Ok(0),
            Some(value) => Ok(serde_json::from_slice(value.as_ref()).map_err(Error::SerdeError)?),
        }
    }
}

fn saturate_path_cache(
    client: &FileMetadata,
    ids: &HashMap<Uuid, FileMetadata>,
    paths: &mut HashMap<Uuid, String>,
) -> Result<String, FindingParentsFailed> {
    match paths.get(&client.id) {
        Some(path) => Ok(path.to_string()),
        None => {
            if client.id == client.parent {
                let path = format!("{}/", client.name.clone());
                paths.insert(client.id, path.clone());
                return Ok(path);
            }
            let parent = ids.get(&client.parent).ok_or(AncestorMissing)?.clone();
            let parent_path = saturate_path_cache(&parent, ids, paths)?;
            let path = match client.file_type {
                FileType::Document => format!("{}{}", parent_path, client.name),
                FileType::Folder => format!("{}{}/", parent_path, client.name),
            };
            paths.insert(client.id, path.clone());
            Ok(path)
        }
    }
}

fn is_leaf_node(id: Uuid, ids: &HashMap<Uuid, FileMetadata>) -> bool {
    match ids.get(&id) {
        None => {
            error!("is_leaf_node was requested an id that wasn't in the list of ids to compute on. id: {:?}, all-ids: {:?}", &id, &ids);
            false
        }
        Some(meta) => {
            if meta.file_type == Document {
                return true;
            }

            for value in ids.values() {
                if value.parent == id {
                    return false;
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::crypto::{EncryptedValueWithNonce, FolderAccessInfo, SignedValue};
    use crate::model::file_metadata::{FileMetadata, FileType};
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn insert_file_metadata() {
        let test_file_metadata = FileMetadata {
            file_type: FileType::Document,
            id: Uuid::new_v4(),
            name: "test".to_string(),
            owner: "".to_string(),
            parent: Default::default(),
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
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        FileMetadataRepoImpl::insert(&db, &test_file_metadata).unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get(&db, test_file_metadata.id).unwrap();
        assert_eq!(test_file_metadata.name, db_file_metadata.name);
        assert_eq!(test_file_metadata.parent, db_file_metadata.parent);

        FileMetadataRepoImpl::maybe_get(&db, test_file_metadata.id)
            .unwrap()
            .unwrap();
        assert!(FileMetadataRepoImpl::maybe_get(&db, Uuid::new_v4())
            .unwrap()
            .is_none());
    }

    #[test]
    fn update_file_metadata() {
        let id = Uuid::new_v4();
        let parent = Uuid::new_v4();
        let test_meta = FileMetadata {
            file_type: FileType::Document,
            id,
            name: "".to_string(),
            owner: "".to_string(),
            parent,
            content_version: 0,
            metadata_version: 0,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: Uuid::new_v4(),
                access_key: EncryptedValueWithNonce {
                    garbage: "".to_string(),
                    nonce: "".to_string(),
                },
            },
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };
        let test_meta_updated = FileMetadata {
            file_type: FileType::Document,
            id,
            name: "".to_string(),
            owner: "".to_string(),
            parent,
            content_version: 1000,
            metadata_version: 1000,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: Uuid::new_v4(),
                access_key: EncryptedValueWithNonce {
                    garbage: "".to_string(),
                    nonce: "".to_string(),
                },
            },
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
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

    #[test]
    fn test_searches() {
        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        let root_id = Uuid::new_v4();

        let root = &FileMetadata {
            file_type: FileType::Folder,
            id: root_id,
            name: "root_folder".to_string(),
            owner: "".to_string(),
            parent: root_id,
            content_version: 0,
            metadata_version: 0,
            user_access_keys: Default::default(),
            folder_access_keys: FolderAccessInfo {
                folder_id: root_id,
                access_key: EncryptedValueWithNonce {
                    garbage: "".to_string(),
                    nonce: "".to_string(),
                },
            },
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };

        let test_file = &FileMetadata {
            file_type: FileType::Document,
            id: Uuid::new_v4(),
            name: "test.txt".to_string(),
            owner: "".to_string(),
            parent: root.id,
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
            deleted: true,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };

        let test_folder = &FileMetadata {
            file_type: FileType::Folder,
            id: Uuid::new_v4(),
            name: "tests".to_string(),
            owner: "".to_string(),
            parent: root.id,
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
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };

        let test_file2 = &FileMetadata {
            file_type: FileType::Document,
            id: Uuid::new_v4(),
            name: "test.txt".to_string(),
            owner: "".to_string(),
            parent: test_folder.id,
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
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };
        let test_file3 = &FileMetadata {
            file_type: FileType::Document,
            id: Uuid::new_v4(),
            name: "test.txt".to_string(),
            owner: "".to_string(),
            parent: test_folder.id,
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
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };
        let test_file4 = &FileMetadata {
            file_type: FileType::Document,
            id: Uuid::new_v4(),
            name: "test.txt".to_string(),
            owner: "".to_string(),
            parent: test_folder.id,
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
            deleted: false,
            signature: SignedValue {
                content: "".to_string(),
                signature: "".to_string(),
            },
        };

        FileMetadataRepoImpl::insert(&db, &root).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_file).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_folder).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_file2).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_file3).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_file4).unwrap();

        let parents = FileMetadataRepoImpl::get_with_all_parents(&db, test_file4.id).unwrap();

        assert_eq!(parents.len(), 3);
        assert!(parents.contains_key(&root.id));
        assert!(parents.contains_key(&test_folder.id));
        assert!(parents.contains_key(&test_file4.id));

        let children = FileMetadataRepoImpl::get_children(&db, root.id).unwrap();
        assert_eq!(children.len(), 2);
    }
}
/*
TODO validations we may want to add here:
1. Don't insert a file with a non existent parent -- causes problems for sync so maybe not
2. Don't insert a file as a child to a document
3. Don't insert a file with a name shared by another file with the same parent (files vs folders?)
4. Don't delete a folder with children, or delete all children when you delete a folder
5. File names should not contain `/` otherwise it'll mess up path parsing
 */
