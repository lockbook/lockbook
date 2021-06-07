use std::collections::HashMap;

use uuid::Uuid;

use crate::model::state::Config;
use crate::repo::file_metadata_repo::FindingParentsFailed::AncestorMissing;
use crate::repo::local_storage;
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};

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

pub static FILE_METADATA: &[u8; 13] = b"file_metadata";
static ROOT: &[u8; 4] = b"ROOT";
static LAST_UPDATED: &[u8; 12] = b"last_updated";

pub fn insert(config: &Config, file: &FileMetadata) -> Result<(), DbError> {
    local_storage::write(
        config,
        FILE_METADATA,
        file.id.to_string().as_str(),
        serde_json::to_vec(&file).map_err(DbError::SerdeError)?,
    )
    .map_err(DbError::BackendError)?;
    if file.id == file.parent {
        debug!("saving root folder: {:?}", &file.id);
        local_storage::write(config, ROOT, ROOT, file.id.to_string().as_str())
            .map_err(DbError::BackendError)?;
    }
    Ok(())
}

pub fn get_root(config: &Config) -> Result<Option<FileMetadata>, DbError> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, ROOT, ROOT).map_err(DbError::BackendError)?;
    match maybe_value {
        None => Ok(None),
        Some(value) => match String::from_utf8(value.clone()) {
            Ok(id) => match Uuid::parse_str(&id) {
                Ok(uuid) => maybe_get(&config, uuid),
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

pub fn get(config: &Config, id: Uuid) -> Result<FileMetadata, GetError> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, FILE_METADATA, id.to_string().as_str())
            .map_err(DbError::BackendError)
            .map_err(GetError::DbError)?;
    let value = maybe_value.ok_or(GetError::FileRowMissing)?;
    let file_metadata: FileMetadata = serde_json::from_slice(value.as_ref())
        .map_err(DbError::SerdeError)
        .map_err(GetError::DbError)?;
    Ok(file_metadata)
}

pub fn maybe_get(config: &Config, id: Uuid) -> Result<Option<FileMetadata>, DbError> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, FILE_METADATA, id.to_string().as_str())
            .map_err(DbError::BackendError)?;
    Ok(maybe_value.and_then(|value| {
        serde_json::from_slice(value.as_ref())
            .map_err(DbError::SerdeError)
            .ok()?
    }))
}

pub fn get_with_all_parents(
    config: &Config,
    id: Uuid,
) -> Result<HashMap<Uuid, FileMetadata>, FindingParentsFailed> {
    let mut parents = HashMap::new();
    let mut current_id = id;
    debug!("Finding parents for: {}", current_id);

    loop {
        match maybe_get(config, current_id).map_err(FindingParentsFailed::DbError)? {
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

#[derive(Debug)]
pub enum FindingChildrenFailed {
    FileDoesNotExist,
    DocumentTreatedAsFolder,
    DbError(DbError),
}

pub fn get_and_get_children_recursively(
    config: &Config,
    id: Uuid,
) -> Result<Vec<FileMetadata>, FindingChildrenFailed> {
    let all = get_all(config).map_err(FindingChildrenFailed::DbError)?;
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

pub fn get_all(config: &Config) -> Result<Vec<FileMetadata>, DbError> {
    let files = local_storage::dump::<_, Vec<u8>>(config, FILE_METADATA)
        .map_err(DbError::BackendError)?
        .into_iter()
        .map(|s| serde_json::from_slice(s.as_ref()).map_err(DbError::SerdeError))
        .collect::<Result<Vec<FileMetadata>, DbError>>();

    let mut files = files?;
    files.retain(|file| !file.deleted);

    Ok(files)
}

pub fn non_recursive_delete(config: &Config, id: Uuid) -> Result<(), DbError> {
    local_storage::delete(config, FILE_METADATA, id.to_string().as_str())
        .map_err(DbError::BackendError)
}

pub fn get_children_non_recursively(
    config: &Config,
    id: Uuid,
) -> Result<Vec<FileMetadata>, DbError> {
    Ok(get_all(config)?
        .into_iter()
        .filter(|file| file.parent == id && file.parent != file.id)
        .collect::<Vec<FileMetadata>>())
}

pub fn set_last_synced(config: &Config, last_updated: u64) -> Result<(), DbError> {
    debug!("Setting last updated to: {}", last_updated);
    local_storage::write(
        config,
        LAST_UPDATED,
        LAST_UPDATED,
        serde_json::to_vec(&last_updated).map_err(DbError::SerdeError)?,
    )
    .map_err(DbError::BackendError)
}

pub fn get_last_updated(config: &Config) -> Result<u64, DbError> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, LAST_UPDATED, LAST_UPDATED).map_err(DbError::BackendError)?;
    match maybe_value {
        None => Ok(0),
        Some(value) => Ok(serde_json::from_slice(value.as_ref()).map_err(DbError::SerdeError)?),
    }
}

fn is_leaf_node(config: &Config, id: Uuid) -> Result<bool, DbError> {
    let mut files = get_all(&config)?;
    files.retain(|f| f.parent == id);
    Ok(files.is_empty())
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::state::{temp_config, Config};
    use crate::repo::{account_repo, file_metadata_repo};
    use crate::service::{file_encryption_service, file_service};
    use lockbook_crypto::{pubkey, symkey};
    use lockbook_models::account::Account;
    use lockbook_models::crypto::EncryptedFolderAccessKey;
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use lockbook_models::file_metadata::{FileMetadata, FileType};

    fn base_test_file_metadata() -> FileMetadata {
        FileMetadata {
            file_type: FileType::Document,
            id: Uuid::new_v4(),
            name: symkey::encrypt_and_hmac(&symkey::generate_key(), "dummy").unwrap(),
            owner: "".to_string(),
            parent: Default::default(),
            content_version: 0,
            metadata_version: 0,
            user_access_keys: Default::default(),
            folder_access_keys: EncryptedFolderAccessKey::new("", ""),
            deleted: false,
        }
    }

    fn insert_test_metadata_root(config: &Config) -> FileMetadata {
        let root_id = Uuid::new_v4();
        let fmd = FileMetadata {
            file_type: FileType::Folder,
            id: root_id,
            parent: root_id,
            ..base_test_file_metadata()
        };
        file_metadata_repo::insert(config, &fmd).unwrap();
        fmd
    }

    fn insert_test_metadata(config: &Config, file_type: FileType, parent: Uuid) -> FileMetadata {
        let fmd = FileMetadata {
            file_type,
            id: Uuid::new_v4(),
            parent,
            ..base_test_file_metadata()
        };
        file_metadata_repo::insert(config, &fmd).unwrap();
        fmd
    }

    #[test]
    fn insert_file_metadata() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config);
        let test_file = insert_test_metadata(config, FileType::Document, root.id);

        let retrieved_file_metadata = file_metadata_repo::get(config, test_file.id).unwrap();
        assert_eq!(test_file.name, retrieved_file_metadata.name);
        assert_eq!(test_file.parent, retrieved_file_metadata.parent);

        file_metadata_repo::maybe_get(config, test_file.id)
            .unwrap()
            .unwrap();
        assert!(file_metadata_repo::maybe_get(config, Uuid::new_v4())
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

        file_metadata_repo::insert(config, &test_meta).unwrap();
        assert_eq!(
            test_meta.content_version,
            file_metadata_repo::get(config, test_meta.id)
                .unwrap()
                .content_version
        );
        file_metadata_repo::insert(config, &test_meta_updated).unwrap();
        assert_eq!(
            test_meta_updated.content_version,
            file_metadata_repo::get(config, test_meta_updated.id)
                .unwrap()
                .content_version
        );
    }

    #[test]
    fn test_searches() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config);
        let _ = insert_test_metadata(config, FileType::Document, root.id);
        let test_folder = insert_test_metadata(config, FileType::Folder, root.id);
        let _ = insert_test_metadata(config, FileType::Document, test_folder.id);
        let _ = insert_test_metadata(config, FileType::Document, test_folder.id);
        let test_file4 = insert_test_metadata(config, FileType::Document, test_folder.id);

        let parents = file_metadata_repo::get_with_all_parents(config, test_file4.id).unwrap();
        assert_eq!(parents.len(), 3);
        assert!(parents.contains_key(&root.id));
        assert!(parents.contains_key(&test_folder.id));
        assert!(parents.contains_key(&test_file4.id));

        let children = file_metadata_repo::get_children_non_recursively(config, root.id).unwrap();
        assert_eq!(children.len(), 2);
    }

    // #[test]
    // fn test_integrity_no_problems() {
    //     let config = &temp_config();

    //     let _ = insert_test_metadata_root(config, "rootdir");

    //     let probs = file_metadata_repo::test_repo_integrity(config).unwrap();
    //     assert!(probs.is_empty());
    // }

    // #[test]
    // fn test_no_root() {
    //     let config = temp_config();

    //     let probs = file_metadata_repo::test_repo_integrity(&config).unwrap();
    //     assert_eq!(probs.len(), 1);
    //     assert_eq!(probs.get(0).unwrap(), &Problem::NoRootFolder);
    // }

    // #[test]
    // fn test_orphaned_children() {
    //     let config = &temp_config();

    //     let keys = pubkey::generate_key();

    //     let account = Account {
    //         username: String::from("username"),
    //         api_url: "ftp://uranus.net".to_string(),
    //         private_key: keys,
    //     };

    //     account_repo::insert_account(config, &account).unwrap();
    //     let root = file_encryption_service::create_metadata_for_root_folder(&account).unwrap();
    //     file_metadata_repo::insert(config, &root).unwrap();

    //     file_service::create_at_path(config, "username/folder1/file1.txt").unwrap();
    //     file_service::create_at_path(config, "username/folder2/file2.txt").unwrap();
    //     file_service::create_at_path(config, "username/folder2/file3.txt").unwrap();
    //     file_service::create_at_path(config, "username/folder2/file4.txt").unwrap();
    //     file_service::create_at_path(config, "username/folder3/file5.txt").unwrap();

    //     assert!(file_metadata_repo::test_repo_integrity(config)
    //         .unwrap()
    //         .is_empty());

    //     let orphan = insert_test_metadata(config, FileType::Document, Uuid::new_v4(), "test");

    //     let probs = file_metadata_repo::test_repo_integrity(config).unwrap();
    //     assert_eq!(probs.len(), 1);
    //     assert_eq!(probs.get(0).unwrap(), &Problem::FileOrphaned(orphan.id));

    //     let _ = insert_test_metadata(config, FileType::Document, Uuid::new_v4(), "test");

    //     let probs = file_metadata_repo::test_repo_integrity(config).unwrap();
    //     assert_eq!(probs.len(), 2);
    // }

    // #[test]
    // fn test_files_invalid_names() {
    //     let config = &temp_config();

    //     let root = insert_test_metadata_root(config, "rootdir");
    //     let has_slash = insert_test_metadata(config, FileType::Document, root.id, "uh/oh");
    //     let empty_name = insert_test_metadata(config, FileType::Document, root.id, "");

    //     let probs = file_metadata_repo::test_repo_integrity(config).unwrap();
    //     assert_eq!(probs.len(), 2);
    //     assert!(probs.contains(&Problem::FileNameContainsSlash(has_slash.id)));
    //     assert!(probs.contains(&Problem::FileNameEmpty(empty_name.id)));
    // }

    // #[test]
    // fn test_cycle_detection() {
    //     let config = &temp_config();

    //     let _ = insert_test_metadata_root(config, "rootdir");
    //     let folder1 = Uuid::new_v4();
    //     let folder2 = Uuid::new_v4();

    //     file_metadata_repo::insert(
    //         config,
    //         &FileMetadata {
    //             id: folder2,
    //             file_type: FileType::Folder,
    //             parent: folder1,
    //             name: "uhoh".to_string(),
    //             ..base_test_file_metadata()
    //         },
    //     )
    //     .unwrap();

    //     file_metadata_repo::insert(
    //         config,
    //         &FileMetadata {
    //             id: folder1,
    //             file_type: FileType::Folder,
    //             parent: folder2,
    //             name: "uhoh".to_string(),
    //             ..base_test_file_metadata()
    //         },
    //     )
    //     .unwrap();

    //     assert_eq!(
    //         file_metadata_repo::test_repo_integrity(config)
    //             .unwrap()
    //             .into_iter()
    //             .filter(|prob| *prob == CycleDetected(folder1) || *prob == CycleDetected(folder2))
    //             .count(),
    //         2
    //     );
    // }

    // #[test]
    // fn test_name_conflicts() {
    //     let config = &temp_config();

    //     let root = insert_test_metadata_root(config, "uhoh");
    //     let doc1 = insert_test_metadata(config, FileType::Document, root.id, "a");
    //     let doc2 = insert_test_metadata(config, FileType::Document, root.id, "a");

    //     let probs = file_metadata_repo::test_repo_integrity(config).unwrap();
    //     assert_eq!(probs.len(), 1);

    //     let p = probs.get(0).unwrap();
    //     assert!(*p == NameConflictDetected(doc1.id) || *p == NameConflictDetected(doc2.id));
    // }

    // #[test]
    // fn test_document_treated_as_folder() {
    //     let config = &temp_config();

    //     let root = insert_test_metadata_root(config, "uhoh");
    //     let doc = insert_test_metadata(config, FileType::Document, root.id, "a");
    //     let _ = insert_test_metadata(config, FileType::Document, doc.id, "b");

    //     let probs = file_metadata_repo::test_repo_integrity(config).unwrap();
    //     assert_eq!(probs.len(), 1);
    //     assert!(probs.contains(&Problem::DocumentTreatedAsFolder(doc.id)));
    // }

    #[test]
    fn test_get_children_handle_empty_root() {
        let config = &temp_config();
        let root = insert_test_metadata_root(config);
        let children_of_root =
            file_metadata_repo::get_and_get_children_recursively(config, root.id).unwrap();
        assert_eq!(children_of_root, vec![root])
    }

    #[test]
    fn test_get_children() {
        let config = &temp_config();

        let root = insert_test_metadata_root(config);

        let doc = insert_test_metadata(
            config,
            Document,
            file_metadata_repo::get_root(config).unwrap().unwrap().id,
        );

        {
            let mut children_of_root =
                file_metadata_repo::get_and_get_children_recursively(config, root.id).unwrap();
            // TODO assert specific children here.
            assert_eq!(children_of_root.len(), 2);
            assert!(file_metadata_repo::get_and_get_children_recursively(config, doc.id).is_err());
        }

        let folder = insert_test_metadata(
            config,
            Folder,
            file_metadata_repo::get_root(config).unwrap().unwrap().id,
        );

        {
            let mut children_of_root =
                file_metadata_repo::get_and_get_children_recursively(config, root.id).unwrap();
            assert_eq!(children_of_root.len(), 3);
            assert!(file_metadata_repo::get_and_get_children_recursively(config, doc.id).is_err());

            assert_eq!(
                file_metadata_repo::get_and_get_children_recursively(config, folder.id).unwrap(),
                vec![folder.clone()]
            );
        }

        let doc2 = insert_test_metadata(config, Document, folder.id);

        let doc3 = insert_test_metadata(config, Document, folder.id);

        let doc4 = insert_test_metadata(config, Document, folder.id);

        let doc5 = insert_test_metadata(config, Document, folder.id);

        let doc6 = insert_test_metadata(config, Document, folder.id);

        let doc7 = insert_test_metadata(config, Document, folder.id);

        {
            let mut children_of_folder =
                file_metadata_repo::get_and_get_children_recursively(config, folder.id).unwrap();
            assert_eq!(children_of_folder.len(), 7);
        }
    }
}
