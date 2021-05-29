use std::collections::HashMap;

use uuid::Uuid;

use crate::model::state::Config;
use crate::repo::file_metadata_repo::FindingParentsFailed::AncestorMissing;
use crate::repo::file_metadata_repo::Problem::{
    CycleDetected, DocumentTreatedAsFolder, FileNameContainsSlash, FileNameEmpty, FileOrphaned,
    NameConflictDetected, NoRootFolder,
};
use crate::storage::db_provider::FileBackend;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::file_metadata::{FileMetadata, FileType};

#[derive(Debug)]
pub enum DbError {
    BackendError(std::io::Error),
    SerdeError(serde_json::Error),
}

#[derive(Debug)]
pub enum GetError {
    FileRowMissing,
    DbError(DbError),
}

#[derive(Debug)]
pub enum FindingParentsFailed {
    AncestorMissing,
    DbError(DbError),
}

#[derive(Debug)]
pub enum FindingChildrenFailed {
    FileDoesNotExist,
    DocumentTreatedAsFolder,
    DbError(DbError),
}

pub enum Filter {
    DocumentsOnly,
    FoldersOnly,
    LeafNodesOnly,
}

#[derive(Debug)]
pub enum StringToFilterError {
    UnknownFilter,
}

pub fn filter_from_str(input: &str) -> Result<Option<Filter>, StringToFilterError> {
    match input {
        "DocumentsOnly" => Ok(Some(Filter::DocumentsOnly)),
        "FoldersOnly" => Ok(Some(Filter::FoldersOnly)),
        "LeafNodesOnly" => Ok(Some(Filter::LeafNodesOnly)),
        "Unfiltered" => Ok(None),
        _ => Err(StringToFilterError::UnknownFilter),
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
    fn insert(config: &Config, file: &FileMetadata) -> Result<(), DbError>;
    fn get_root(config: &Config) -> Result<Option<FileMetadata>, DbError>;
    fn get(config: &Config, id: Uuid) -> Result<FileMetadata, GetError>;
    fn maybe_get(config: &Config, id: Uuid) -> Result<Option<FileMetadata>, DbError>;
    fn get_by_path(config: &Config, path: &str) -> Result<Option<FileMetadata>, DbError>;
    fn get_with_all_parents(
        config: &Config,
        id: Uuid,
    ) -> Result<HashMap<Uuid, FileMetadata>, FindingParentsFailed>;
    fn get_and_get_children_recursively(
        config: &Config,
        id: Uuid,
    ) -> Result<Vec<FileMetadata>, FindingChildrenFailed>;
    fn get_all(config: &Config) -> Result<Vec<FileMetadata>, DbError>;
    fn get_all_paths(
        config: &Config,
        filter: Option<Filter>,
    ) -> Result<Vec<String>, FindingParentsFailed>;
    fn non_recursive_delete(config: &Config, id: Uuid) -> Result<(), DbError>;
    fn get_children_non_recursively(
        config: &Config,
        id: Uuid,
    ) -> Result<Vec<FileMetadata>, DbError>;
    fn set_last_synced(config: &Config, last_updated: u64) -> Result<(), DbError>;
    fn get_last_updated(config: &Config) -> Result<u64, DbError>;
    fn test_repo_integrity(config: &Config) -> Result<Vec<Problem>, DbError>;
}

pub struct FileMetadataRepoImpl;

pub static FILE_METADATA: &[u8; 13] = b"file_metadata";
static ROOT: &[u8; 4] = b"ROOT";
static LAST_UPDATED: &[u8; 12] = b"last_updated";

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert(config: &Config, file: &FileMetadata) -> Result<(), DbError> {
        FileBackend::write(
            config,
            FILE_METADATA,
            file.id.to_string().as_str(),
            serde_json::to_vec(&file).map_err(DbError::SerdeError)?,
        )
        .map_err(DbError::BackendError)?;
        if file.id == file.parent {
            debug!("saving root folder: {:?}", &file.id);
            FileBackend::write(config, ROOT, ROOT, file.id.to_string().as_str())
                .map_err(DbError::BackendError)?;
        }
        Ok(())
    }

    fn get_root(config: &Config) -> Result<Option<FileMetadata>, DbError> {
        let maybe_value: Option<Vec<u8>> =
            FileBackend::read(config, ROOT, ROOT).map_err(DbError::BackendError)?;
        match maybe_value {
            None => Ok(None),
            Some(value) => match String::from_utf8(value.clone()) {
                Ok(id) => match Uuid::parse_str(&id) {
                    Ok(uuid) => Self::maybe_get(&config, uuid),
                    Err(err) => {
                        error!("Failed to parse {:?} into a UUID. Error: {:?}", id, err);
                        Ok(None)
                    }
                },
                Err(err) => {
                    error!("Failed parsing {:?} into a UUID. Error: {:?}", &value, err);
                    Ok(None)
                }
            },
        }
    }

    fn get(config: &Config, id: Uuid) -> Result<FileMetadata, GetError> {
        let maybe_value: Option<Vec<u8>> =
            FileBackend::read(config, FILE_METADATA, id.to_string().as_str())
                .map_err(DbError::BackendError)
                .map_err(GetError::DbError)?;
        let value = maybe_value.ok_or(GetError::FileRowMissing)?;
        let file_metadata: FileMetadata = serde_json::from_slice(value.as_ref())
            .map_err(DbError::SerdeError)
            .map_err(GetError::DbError)?;
        Ok(file_metadata)
    }

    fn maybe_get(config: &Config, id: Uuid) -> Result<Option<FileMetadata>, DbError> {
        let maybe_value: Option<Vec<u8>> =
            FileBackend::read(config, FILE_METADATA, id.to_string().as_str())
                .map_err(DbError::BackendError)?;
        Ok(maybe_value.and_then(|value| {
            serde_json::from_slice(value.as_ref())
                .map_err(DbError::SerdeError)
                .ok()?
        }))
    }

    fn get_by_path(config: &Config, path: &str) -> Result<Option<FileMetadata>, DbError> {
        debug!("Path: {}", path);
        let root = match Self::get_root(config)? {
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

            let children = Self::get_children_non_recursively(config, current.id)?;
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
        config: &Config,
        id: Uuid,
    ) -> Result<HashMap<Uuid, FileMetadata>, FindingParentsFailed> {
        let mut parents = HashMap::new();
        let mut current_id = id;
        debug!("Finding parents for: {}", current_id);

        loop {
            match Self::maybe_get(config, current_id).map_err(FindingParentsFailed::DbError)? {
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

    fn get_and_get_children_recursively(
        config: &Config,
        id: Uuid,
    ) -> Result<Vec<FileMetadata>, FindingChildrenFailed> {
        let all = Self::get_all(config).map_err(FindingChildrenFailed::DbError)?;
        let target_file = all
            .clone()
            .into_iter()
            .find(|file| file.id == id)
            .ok_or(FindingChildrenFailed::FileDoesNotExist)?;
        let mut result = vec![target_file.clone()];

        if target_file.file_type == Document {
            return Err(FindingChildrenFailed::DocumentTreatedAsFolder);
        }

        let mut to_explore = all
            .clone()
            .into_iter()
            .filter(|file| file.parent == target_file.id && file.id != target_file.id)
            .collect::<Vec<FileMetadata>>();

        while !to_explore.is_empty() {
            let mut explore_next_round = vec![];

            for file in to_explore {
                if file.file_type == Folder {
                    // Explore this folder's children
                    all.clone()
                        .into_iter()
                        .filter(|maybe_child| maybe_child.parent == file.id)
                        .for_each(|f| explore_next_round.push(f));
                }

                result.push(file.clone());
            }

            to_explore = explore_next_round;
        }

        Ok(result)
    }

    fn get_all(config: &Config) -> Result<Vec<FileMetadata>, DbError> {
        let files = FileBackend::dump::<_, Vec<u8>>(config, FILE_METADATA)
            .map_err(DbError::BackendError)?
            .into_iter()
            .map(|s| serde_json::from_slice(s.as_ref()).map_err(DbError::SerdeError))
            .collect::<Result<Vec<FileMetadata>, DbError>>();

        let mut files = files?;
        files.retain(|file| !file.deleted);

        Ok(files)
    }

    fn get_all_paths(
        config: &Config,
        filter: Option<Filter>,
    ) -> Result<Vec<String>, FindingParentsFailed> {
        let mut cache = HashMap::new();
        let mut path_cache = HashMap::new();

        // Populate metadata cache
        Self::get_all(config)
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

    fn non_recursive_delete(config: &Config, id: Uuid) -> Result<(), DbError> {
        FileBackend::delete(config, FILE_METADATA, id.to_string().as_str())
            .map_err(DbError::BackendError)
    }

    fn get_children_non_recursively(
        config: &Config,
        id: Uuid,
    ) -> Result<Vec<FileMetadata>, DbError> {
        Ok(Self::get_all(config)?
            .into_iter()
            .filter(|file| file.parent == id && file.parent != file.id)
            .collect::<Vec<FileMetadata>>())
    }

    fn set_last_synced(config: &Config, last_updated: u64) -> Result<(), DbError> {
        debug!("Setting last updated to: {}", last_updated);
        FileBackend::write(
            config,
            LAST_UPDATED,
            LAST_UPDATED,
            serde_json::to_vec(&last_updated).map_err(DbError::SerdeError)?,
        )
        .map_err(DbError::BackendError)
    }

    fn get_last_updated(config: &Config) -> Result<u64, DbError> {
        let maybe_value: Option<Vec<u8>> =
            FileBackend::read(config, LAST_UPDATED, LAST_UPDATED).map_err(DbError::BackendError)?;
        match maybe_value {
            None => Ok(0),
            Some(value) => Ok(serde_json::from_slice(value.as_ref()).map_err(DbError::SerdeError)?),
        }
    }

    fn test_repo_integrity(config: &Config) -> Result<Vec<Problem>, DbError> {
        let all = Self::get_all(config)?;
        let mut probs = vec![];
        match Self::get_root(config)? {
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
    use uuid::Uuid;

    use crate::model::state::{temp_config, Config};
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::file_metadata_repo::Problem::{CycleDetected, NameConflictDetected};
    use crate::repo::file_metadata_repo::{FileMetadataRepo, Problem};
    use crate::service::file_encryption_service::FileEncryptionService;
    use crate::service::file_service::FileService;
    use crate::{
        DefaultAccountRepo, DefaultFileEncryptionService, DefaultFileMetadataRepo,
        DefaultFileService,
    };
    use lockbook_crypto::pubkey;
    use lockbook_models::account::Account;
    use lockbook_models::crypto::{EncryptedFolderAccessKey, FolderAccessInfo};
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use lockbook_models::file_metadata::{FileMetadata, FileType};

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
                access_key: EncryptedFolderAccessKey::new("", ""),
            },
            deleted: false,
        }
    }

    fn insert_test_metadata_root(config: &Config, name: &str) -> FileMetadata {
        let root_id = Uuid::new_v4();
        let fmd = FileMetadata {
            file_type: FileType::Folder,
            id: root_id,
            name: name.to_string(),
            parent: root_id,
            ..base_test_file_metadata()
        };
        DefaultFileMetadataRepo::insert(config, &fmd).unwrap();
        fmd
    }

    fn insert_test_metadata(
        config: &Config,
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
        DefaultFileMetadataRepo::insert(config, &fmd).unwrap();
        fmd
    }

    #[test]
    fn insert_file_metadata() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config, "root_folder");
        let test_file = insert_test_metadata(config, FileType::Document, root.id, "test.txt");

        let retrieved_file_metadata = DefaultFileMetadataRepo::get(config, test_file.id).unwrap();
        assert_eq!(test_file.name, retrieved_file_metadata.name);
        assert_eq!(test_file.parent, retrieved_file_metadata.parent);

        DefaultFileMetadataRepo::maybe_get(config, test_file.id)
            .unwrap()
            .unwrap();
        assert!(DefaultFileMetadataRepo::maybe_get(config, Uuid::new_v4())
            .unwrap()
            .is_none());
    }

    #[test]
    fn update_file_metadata() {
        let config = &temp_config();

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

        DefaultFileMetadataRepo::insert(config, &test_meta).unwrap();
        assert_eq!(
            test_meta.content_version,
            DefaultFileMetadataRepo::get(config, test_meta.id)
                .unwrap()
                .content_version
        );
        DefaultFileMetadataRepo::insert(config, &test_meta_updated).unwrap();
        assert_eq!(
            test_meta_updated.content_version,
            DefaultFileMetadataRepo::get(config, test_meta_updated.id)
                .unwrap()
                .content_version
        );
    }

    #[test]
    fn test_searches() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config, "root_folder1");
        let _ = insert_test_metadata(config, FileType::Document, root.id, "test.txt");
        let test_folder = insert_test_metadata(config, FileType::Folder, root.id, "test");
        let _ = insert_test_metadata(config, FileType::Document, test_folder.id, "test.txt");
        let _ = insert_test_metadata(config, FileType::Document, test_folder.id, "test.txt");
        let test_file4 =
            insert_test_metadata(config, FileType::Document, test_folder.id, "test.txt");

        let parents = DefaultFileMetadataRepo::get_with_all_parents(config, test_file4.id).unwrap();
        assert_eq!(parents.len(), 3);
        assert!(parents.contains_key(&root.id));
        assert!(parents.contains_key(&test_folder.id));
        assert!(parents.contains_key(&test_file4.id));

        let children =
            DefaultFileMetadataRepo::get_children_non_recursively(config, root.id).unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn test_integrity_no_problems() {
        let config = &temp_config();

        let _ = insert_test_metadata_root(config, "rootdir");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(config).unwrap();
        assert!(probs.is_empty());
    }

    #[test]
    fn test_no_root() {
        let config = temp_config();

        let probs = DefaultFileMetadataRepo::test_repo_integrity(&config).unwrap();
        assert_eq!(probs.len(), 1);
        assert_eq!(probs.get(0).unwrap(), &Problem::NoRootFolder);
    }

    #[test]
    fn test_orphaned_children() {
        let config = &temp_config();

        let keys = pubkey::generate_key();

        let account = Account {
            username: String::from("username"),
            api_url: "ftp://uranus.net".to_string(),
            private_key: keys,
        };

        DefaultAccountRepo::insert_account(config, &account).unwrap();
        let root = DefaultFileEncryptionService::create_metadata_for_root_folder(&account).unwrap();
        DefaultFileMetadataRepo::insert(config, &root).unwrap();

        DefaultFileService::create_at_path(config, "username/folder1/file1.txt").unwrap();
        DefaultFileService::create_at_path(config, "username/folder2/file2.txt").unwrap();
        DefaultFileService::create_at_path(config, "username/folder2/file3.txt").unwrap();
        DefaultFileService::create_at_path(config, "username/folder2/file4.txt").unwrap();
        DefaultFileService::create_at_path(config, "username/folder3/file5.txt").unwrap();

        assert!(DefaultFileMetadataRepo::test_repo_integrity(config)
            .unwrap()
            .is_empty());

        let orphan = insert_test_metadata(config, FileType::Document, Uuid::new_v4(), "test");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(config).unwrap();
        assert_eq!(probs.len(), 1);
        assert_eq!(probs.get(0).unwrap(), &Problem::FileOrphaned(orphan.id));

        let _ = insert_test_metadata(config, FileType::Document, Uuid::new_v4(), "test");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(config).unwrap();
        assert_eq!(probs.len(), 2);
    }

    #[test]
    fn test_files_invalid_names() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config, "rootdir");
        let has_slash = insert_test_metadata(config, FileType::Document, root.id, "uh/oh");
        let empty_name = insert_test_metadata(config, FileType::Document, root.id, "");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(config).unwrap();
        assert_eq!(probs.len(), 2);
        assert!(probs.contains(&Problem::FileNameContainsSlash(has_slash.id)));
        assert!(probs.contains(&Problem::FileNameEmpty(empty_name.id)));
    }

    #[test]
    fn test_cycle_detection() {
        let config = &temp_config();

        let _ = insert_test_metadata_root(config, "rootdir");
        let folder1 = Uuid::new_v4();
        let folder2 = Uuid::new_v4();

        DefaultFileMetadataRepo::insert(
            config,
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
            config,
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
            DefaultFileMetadataRepo::test_repo_integrity(config)
                .unwrap()
                .into_iter()
                .filter(|prob| *prob == CycleDetected(folder1) || *prob == CycleDetected(folder2))
                .count(),
            2
        );
    }

    #[test]
    fn test_name_conflicts() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config, "uhoh");
        let doc1 = insert_test_metadata(config, FileType::Document, root.id, "a");
        let doc2 = insert_test_metadata(config, FileType::Document, root.id, "a");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(config).unwrap();
        assert_eq!(probs.len(), 1);

        let p = probs.get(0).unwrap();
        assert!(*p == NameConflictDetected(doc1.id) || *p == NameConflictDetected(doc2.id));
    }

    #[test]
    fn test_document_treated_as_folder() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config, "uhoh");
        let doc = insert_test_metadata(config, FileType::Document, root.id, "a");
        let _ = insert_test_metadata(config, FileType::Document, doc.id, "b");

        let probs = DefaultFileMetadataRepo::test_repo_integrity(config).unwrap();
        assert_eq!(probs.len(), 1);
        assert!(probs.contains(&Problem::DocumentTreatedAsFolder(doc.id)));
    }

    #[test]
    fn test_get_children_handle_empty_root() {
        let config = &temp_config();
        let root = insert_test_metadata_root(config, "root");
        let children_of_root =
            DefaultFileMetadataRepo::get_and_get_children_recursively(config, root.id).unwrap();
        assert_eq!(children_of_root, vec![root])
    }

    #[test]
    fn test_get_children() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config, "root");

        let doc = insert_test_metadata(
            config,
            Document,
            DefaultFileMetadataRepo::get_root(config)
                .unwrap()
                .unwrap()
                .id,
            "child doc",
        );

        {
            let mut children_of_root =
                DefaultFileMetadataRepo::get_and_get_children_recursively(config, root.id).unwrap();
            children_of_root.sort_by(|f1, f2| f1.name.cmp(&f2.name));
            assert_eq!(children_of_root, vec![doc.clone(), root.clone()]);
            assert!(
                DefaultFileMetadataRepo::get_and_get_children_recursively(config, doc.id).is_err()
            );
        }

        let folder = insert_test_metadata(
            config,
            Folder,
            DefaultFileMetadataRepo::get_root(config)
                .unwrap()
                .unwrap()
                .id,
            "child folder",
        );

        {
            let mut children_of_root =
                DefaultFileMetadataRepo::get_and_get_children_recursively(config, root.id).unwrap();
            children_of_root.sort_by(|f1, f2| f1.name.cmp(&f2.name));
            assert_eq!(
                children_of_root,
                vec![doc.clone(), folder.clone(), root.clone()]
            );
            assert!(
                DefaultFileMetadataRepo::get_and_get_children_recursively(config, doc.id).is_err()
            );

            assert_eq!(
                DefaultFileMetadataRepo::get_and_get_children_recursively(config, folder.id)
                    .unwrap(),
                vec![folder.clone()]
            );
        }

        let doc2 = insert_test_metadata(config, Document, folder.id, "child doc2");

        let doc3 = insert_test_metadata(config, Document, folder.id, "child doc3");

        let doc4 = insert_test_metadata(config, Document, folder.id, "child doc4");

        let doc5 = insert_test_metadata(config, Document, folder.id, "child doc5");

        let doc6 = insert_test_metadata(config, Document, folder.id, "child doc6");

        let doc7 = insert_test_metadata(config, Document, folder.id, "child doc7");

        {
            let mut children_of_folder =
                DefaultFileMetadataRepo::get_and_get_children_recursively(config, folder.id)
                    .unwrap();
            children_of_folder.sort_by(|f1, f2| f1.name.cmp(&f2.name));
            assert_eq!(
                children_of_folder,
                vec![
                    doc2.clone(),
                    doc3.clone(),
                    doc4.clone(),
                    doc5.clone(),
                    doc6.clone(),
                    doc7.clone(),
                    folder.clone()
                ]
            );
        }
    }
}
