use std::collections::HashMap;

use sled::Db;
use uuid::Uuid;

use crate::model::file_metadata::FileType::{Document, Folder};
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::repo::file_metadata_repo::FindingParentsFailed::AncestorMissing;
use crate::repo::file_metadata_repo::Problem::{
    CycleDetected, DocumentTreatedAsFolder, FileNameContainsSlash, FileNameEmpty, FileOrphaned,
    NameConflictDetected, NoRootFolder,
};

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

impl From<sled::Error> for DbError {
    fn from(err: sled::Error) -> Self {
        Self::SledError(err)
    }
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

pub fn filter_from_str(input: &str) -> Result<Option<Filter>, ()> {
    match input {
        "DocumentsOnly" => Ok(Some(Filter::DocumentsOnly)),
        "FoldersOnly" => Ok(Some(Filter::FoldersOnly)),
        "LeafNodesOnly" => Ok(Some(Filter::LeafNodesOnly)),
        "Unfiltered" => Ok(None),
        _ => Err(()),
    }
}

#[derive(Debug, PartialEq)]
pub enum Problem {
    NoRootFolder,
    FileOrphaned(Uuid),
    FileNameEmpty(Uuid),
    FileNameContainsSlash(Uuid),
    CycleDetected(Uuid),
    NameConflictDetected(Uuid),
    DocumentTreatedAsFolder(Uuid),
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
    fn set_last_synced(db: &Db, last_updated: u64) -> Result<(), DbError>;
    fn get_last_updated(db: &Db) -> Result<u64, DbError>;
    fn test_repo_integrity(db: &Db) -> Result<Vec<Problem>, DbError>;
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
        let value: Result<Vec<_>, _> = tree
            .iter()
            .map(|s| serde_json::from_slice(s?.1.as_ref()).map_err(DbError::SerdeError))
            .collect();
        value
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

    fn get_children(db: &Db, id: Uuid) -> Result<Vec<FileMetadata>, DbError> {
        Ok(Self::get_all(&db)?
            .into_iter()
            .filter(|file| file.parent == id && file.parent != file.id)
            .collect::<Vec<FileMetadata>>())
    }

    fn set_last_synced(db: &Db, last_updated: u64) -> Result<(), DbError> {
        debug!("Setting last updated to: {}", last_updated);
        let tree = db.open_tree(LAST_UPDATED).map_err(DbError::SledError)?;
        tree.insert(
            LAST_UPDATED,
            serde_json::to_vec(&last_updated).map_err(DbError::SerdeError)?,
        )
        .map_err(DbError::SledError)?;
        Ok(())
    }

    fn get_last_updated(db: &Db) -> Result<u64, DbError> {
        let tree = db.open_tree(LAST_UPDATED).map_err(DbError::SledError)?;
        let maybe_value = tree.get(LAST_UPDATED).map_err(DbError::SledError)?;
        match maybe_value {
            None => Ok(0),
            Some(value) => Ok(serde_json::from_slice(value.as_ref()).map_err(DbError::SerdeError)?),
        }
    }

    fn test_repo_integrity(db: &Db) -> Result<Vec<Problem>, DbError> {
        let all = Self::get_all(&db)?;
        let mut probs = vec![];
        match Self::get_root(&db)? {
            None => {
                if all.is_empty() {
                    probs.push(NoRootFolder);
                } else {
                    for file in all {
                        probs.push(FileOrphaned(file.id));
                        if file.name.is_empty() {
                            probs.push(FileNameEmpty(file.id));
                        } else if file.name.contains('/') {
                            probs.push(FileNameContainsSlash(file.id));
                        }
                    }
                }
            }
            Some(root) => {
                let mut cache = HashMap::new();

                // Saturate a cache
                for file in all.clone() {
                    cache.insert(file.id, file);
                }

                // Find files with invalid names
                for file in all.clone() {
                    if file.name.is_empty() {
                        probs.push(FileNameEmpty(file.id));
                    } else if file.name.contains('/') {
                        probs.push(FileNameContainsSlash(file.id));
                    }
                }

                // Find naming conflicts
                {
                    let mut children = HashMap::new();
                    for file in all.clone() {
                        if children.contains_key(&format!(
                            "{}.{}",
                            file.parent.to_string(),
                            file.name
                        )) {
                            probs.push(NameConflictDetected(file.id));
                        }
                        children.insert(format!("{}.{}", file.parent, file.name), file.file_type);
                    }
                }

                // Find Documents treated as Folders
                for file in all.clone() {
                    if file.file_type == Document {
                        for potential_child in all.clone() {
                            if file.id == potential_child.parent {
                                probs.push(DocumentTreatedAsFolder(file.id));
                            }
                        }
                    }
                }

                // Find files that don't descend from root
                {
                    let mut not_orphaned = HashMap::new();
                    not_orphaned.insert(root.id, root);

                    for file in all.clone() {
                        let mut visited: HashMap<Uuid, FileMetadata> = HashMap::new();
                        let mut current = file.clone();
                        'parent_finder: loop {
                            if visited.contains_key(&current.id) {
                                probs.push(CycleDetected(current.id));
                                break 'parent_finder;
                            }
                            visited.insert(current.id, current.clone());

                            match cache.get(&current.parent) {
                                None => {
                                    probs.push(FileOrphaned(current.id));
                                    break 'parent_finder;
                                }
                                Some(parent) => {
                                    // No Problems
                                    if not_orphaned.contains_key(&parent.id) {
                                        for node in visited.values() {
                                            not_orphaned.insert(node.id, node.clone());
                                        }

                                        break 'parent_finder;
                                    } else {
                                        current = parent.clone();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(probs)
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
    use sled::Db;
    use uuid::Uuid;

    use crate::model::account::Account;
    use crate::model::crypto::{EncryptedValueWithNonce, FolderAccessInfo, SignedValue};
    use crate::model::file_metadata::{FileMetadata, FileType};
    use crate::model::state::dummy_config;
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::Problem::{CycleDetected, NameConflictDetected};
    use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl, Problem};
    use crate::service::crypto_service::PubKeyCryptoService;
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::service::file_service::FileService;
    use crate::{
        DefaultAccountRepo, DefaultCrypto, DefaultFileEncryptionService, DefaultFileMetadataRepo,
        DefaultFileService,
    };

    type DefaultDbProvider = TempBackedDB;

    fn base_test_file_metadata() -> FileMetadata {
        FileMetadata {
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
        }
    }

    fn insert_test_metadata_root(db: &Db, name: &str) -> FileMetadata {
        let root_id = Uuid::new_v4();
        let fmd = FileMetadata {
            file_type: FileType::Folder,
            id: root_id,
            name: name.to_string(),
            parent: root_id,
            ..base_test_file_metadata()
        };
        FileMetadataRepoImpl::insert(db, &fmd).unwrap();
        fmd
    }

    fn insert_test_metadata(
        db: &Db,
        file_type: FileType,
        parent: Uuid,
        name: &str,
    ) -> FileMetadata {
        let fmd = FileMetadata {
            file_type,
            id: Uuid::new_v4(),
            name: name.to_string(),
            parent,
            ..base_test_file_metadata()
        };
        FileMetadataRepoImpl::insert(db, &fmd).unwrap();
        fmd
    }

    #[test]
    fn insert_file_metadata() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let root = insert_test_metadata_root(&db, "root_folder");
        let test_file = insert_test_metadata(&db, FileType::Document, root.id, "test.txt");

        let retrieved_file_metadata = FileMetadataRepoImpl::get(&db, test_file.id).unwrap();
        assert_eq!(test_file.name, retrieved_file_metadata.name);
        assert_eq!(test_file.parent, retrieved_file_metadata.parent);

        FileMetadataRepoImpl::maybe_get(&db, test_file.id)
            .unwrap()
            .unwrap();
        assert!(FileMetadataRepoImpl::maybe_get(&db, Uuid::new_v4())
            .unwrap()
            .is_none());
    }

    #[test]
    fn update_file_metadata() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let id = Uuid::new_v4();
        let parent = Uuid::new_v4();

        let test_meta = FileMetadata {
            id,
            parent,
            ..base_test_file_metadata()
        };
        let test_meta_updated = FileMetadata {
            id,
            parent,
            content_version: 1000,
            metadata_version: 1000,
            ..base_test_file_metadata()
        };

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
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let root = insert_test_metadata_root(&db, "root_folder1");
        let _ = insert_test_metadata(&db, FileType::Document, root.id, "test.txt");
        let test_folder = insert_test_metadata(&db, FileType::Folder, root.id, "test");
        let _ = insert_test_metadata(&db, FileType::Document, test_folder.id, "test.txt");
        let _ = insert_test_metadata(&db, FileType::Document, test_folder.id, "test.txt");
        let test_file4 = insert_test_metadata(&db, FileType::Document, test_folder.id, "test.txt");

        let parents = FileMetadataRepoImpl::get_with_all_parents(&db, test_file4.id).unwrap();
        assert_eq!(parents.len(), 3);
        assert!(parents.contains_key(&root.id));
        assert!(parents.contains_key(&test_folder.id));
        assert!(parents.contains_key(&test_file4.id));

        let children = FileMetadataRepoImpl::get_children(&db, root.id).unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn test_integrity_no_problems() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();
        let _ = insert_test_metadata_root(&db, "rootdir");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(&db).unwrap();
        assert!(probs.is_empty());
    }

    #[test]
    fn test_no_root() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();
        let probs = DefaultFileMetadataRepo::test_repo_integrity(&db).unwrap();
        assert!(probs.len() == 1);
        assert!(probs.get(0).unwrap() == &Problem::NoRootFolder);
    }

    #[test]
    fn test_orphaned_children() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let keys = DefaultCrypto::generate_key().unwrap();

        let account = Account {
            username: String::from("username"),
            keys,
        };

        DefaultAccountRepo::insert_account(&db, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(&db, &root).unwrap();

        DefaultFileService::create_at_path(&db, "username/folder1/file1.txt").unwrap();
        DefaultFileService::create_at_path(&db, "username/folder2/file2.txt").unwrap();
        DefaultFileService::create_at_path(&db, "username/folder2/file3.txt").unwrap();
        DefaultFileService::create_at_path(&db, "username/folder2/file4.txt").unwrap();
        DefaultFileService::create_at_path(&db, "username/folder3/file5.txt").unwrap();

        assert!(DefaultFileMetadataRepo::test_repo_integrity(&db)
            .unwrap()
            .is_empty());

        let orphan = insert_test_metadata(&db, FileType::Document, Uuid::new_v4(), "test");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(&db).unwrap();
        assert!(probs.len() == 1);
        assert_eq!(probs.get(0).unwrap(), &Problem::FileOrphaned(orphan.id));

        let _ = insert_test_metadata(&db, FileType::Document, Uuid::new_v4(), "test");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(&db).unwrap();
        assert!(probs.len() == 2);
    }

    #[test]
    fn test_files_invalid_names() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let root = insert_test_metadata_root(&db, "rootdir");
        let has_slash = insert_test_metadata(&db, FileType::Document, root.id, "uh/oh");
        let empty_name = insert_test_metadata(&db, FileType::Document, root.id, "");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(&db).unwrap();
        assert_eq!(probs.len(), 2);
        assert!(probs.contains(&Problem::FileNameContainsSlash(has_slash.id)));
        assert!(probs.contains(&Problem::FileNameEmpty(empty_name.id)));
    }

    #[test]
    fn test_cycle_detection() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let _ = insert_test_metadata_root(&db, "rootdir");
        let folder1 = Uuid::new_v4();
        let folder2 = Uuid::new_v4();

        DefaultFileMetadataRepo::insert(
            &db,
            &FileMetadata {
                id: folder2,
                file_type: FileType::Folder,
                parent: folder1,
                name: "uhoh".to_string(),
                ..base_test_file_metadata()
            },
        )
        .unwrap();

        DefaultFileMetadataRepo::insert(
            &db,
            &FileMetadata {
                id: folder1,
                file_type: FileType::Folder,
                parent: folder2,
                name: "uhoh".to_string(),
                ..base_test_file_metadata()
            },
        )
        .unwrap();

        assert_eq!(
            DefaultFileMetadataRepo::test_repo_integrity(&db)
                .unwrap()
                .into_iter()
                .filter(|prob| *prob == CycleDetected(folder1) || *prob == CycleDetected(folder2))
                .count(),
            2
        );
    }

    #[test]
    fn test_name_conflicts() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let root = insert_test_metadata_root(&db, "uhoh");
        let doc1 = insert_test_metadata(&db, FileType::Document, root.id, "a");
        let doc2 = insert_test_metadata(&db, FileType::Document, root.id, "a");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(&db).unwrap();
        assert!(probs.len() == 1);

        let p = probs.get(0).unwrap();
        assert!(*p == NameConflictDetected(doc1.id) || *p == NameConflictDetected(doc2.id));
    }

    #[test]
    fn test_document_treated_as_folder() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();

        let root = insert_test_metadata_root(&db, "uhoh");
        let doc = insert_test_metadata(&db, FileType::Document, root.id, "a");
        let _ = insert_test_metadata(&db, FileType::Document, doc.id, "b");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(&db).unwrap();
        assert!(probs.len() == 1);
        assert!(probs.contains(&Problem::DocumentTreatedAsFolder(doc.id)));
    }
}
