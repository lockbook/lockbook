use uuid::Uuid;

use lockbook_models::file_metadata::DecryptedFileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::tree::FileMetaExt;

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::pure_functions::files;
use crate::repo::account_repo;

use crate::service::file_service;
use crate::CoreError;

pub fn create_at_path(
    config: &Config, path_and_name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    info!("creating path at: {}", path_and_name);

    if path_and_name.contains("//") {
        return Err(CoreError::PathContainsEmptyFileName);
    }
    let path_components = split_path(path_and_name);

    let is_folder = path_and_name.ends_with('/');

    let mut files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let mut current = files.find_root()?;
    let root_id = current.id;
    let account = account_repo::get(config)?;

    if current.decrypted_name != path_components[0] {
        return Err(CoreError::PathStartsWithNonRoot);
    }

    if path_components.len() == 1 {
        return Err(CoreError::PathTaken);
    }

    // We're going to look ahead, and find or create the right child
    'path: for index in 0..path_components.len() - 1 {
        let children = files.find_children(current.id);

        let next_name = path_components[index + 1];

        for child in children {
            if child.decrypted_name == next_name {
                // If we're at the end and we find this child, that means this path already exists
                if child.id != root_id && index == path_components.len() - 2 {
                    return Err(CoreError::PathTaken);
                }

                if child.file_type == Folder {
                    current = child;
                    continue 'path; // Child exists, onto the next one
                } else {
                    return Err(CoreError::FileNotFolder);
                }
            }
        }

        // Child does not exist, create it
        let file_type =
            if is_folder || index != path_components.len() - 2 { Folder } else { Document };

        current =
            files::apply_create(&files, file_type, current.id, next_name, &account.public_key())?;
        files.push(current.clone());
        file_service::insert_metadatum(config, RepoSource::Local, &current)?;
    }

    Ok(current)
}

pub fn get_by_path(config: &Config, path: &str) -> Result<DecryptedFileMetadata, CoreError> {
    let files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    get_by_path_common(&files, path)
}

pub fn get_by_path_include_deleted(
    config: &Config,
    path: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let files = file_service::get_all_metadata(config, RepoSource::Local)?;
    get_by_path_common(&files, path)
}

fn get_by_path_common(
    files: &Vec<DecryptedFileMetadata>,
    path: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    info!("getting metadata at path: {}", path);
    let paths = split_path(path);

    let mut current = files.find_root()?;

    for (i, &value) in paths.iter().enumerate() {
        if value != current.decrypted_name {
            return Err(CoreError::FileNonexistent);
        }

        if i + 1 == paths.len() {
            return Ok(current);
        }

        let children = files.find_children(current.id);
        let mut found_child = false;

        for child in children {
            if child.decrypted_name == paths[i + 1] {
                current = child;
                found_child = true;
            }
        }

        if !found_child {
            return Err(CoreError::FileNonexistent);
        }
    }

    Ok(current)
}

#[derive(Debug)]
pub enum Filter {
    DocumentsOnly,
    FoldersOnly,
    LeafNodesOnly,
}

pub fn filter_from_str(input: &str) -> Result<Option<Filter>, CoreError> {
    match input {
        "DocumentsOnly" => Ok(Some(Filter::DocumentsOnly)),
        "FoldersOnly" => Ok(Some(Filter::FoldersOnly)),
        "LeafNodesOnly" => Ok(Some(Filter::LeafNodesOnly)),
        "Unfiltered" => Ok(None),
        _ => Err(CoreError::Unexpected(String::from("unknown filter"))),
    }
}

pub fn get_all_paths(config: &Config, filter: Option<Filter>) -> Result<Vec<String>, CoreError> {
    info!("listing all paths with filter {:?}", filter);
    let files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;

    let mut filtered_files = files.clone();

    if let Some(filter) = filter {
        match filter {
            Filter::DocumentsOnly => filtered_files.retain(|f| f.file_type == Document),
            Filter::FoldersOnly => filtered_files.retain(|f| f.file_type == Folder),
            Filter::LeafNodesOnly => {
                filtered_files.retain(|parent| !files.iter().any(|child| child.parent == parent.id))
            }
        }
    }

    let mut paths: Vec<String> = vec![];
    for file in filtered_files {
        let mut current = file.clone();
        let mut current_path = String::from("");
        while current.id != current.parent {
            if current.file_type == Document {
                current_path = current.decrypted_name;
            } else {
                current_path = format!("{}/{}", current.decrypted_name, current_path);
            }
            current = files.find(current.parent)?;
        }

        current_path = format!("{}/{}", current.decrypted_name, current_path);
        paths.push(current_path.to_string());
    }

    Ok(paths)
}

pub fn get_path_by_id(config: &Config, id: Uuid) -> Result<String, CoreError> {
    info!("getting path by id: {}", id);
    let files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let mut current_metadata = files.find(id)?;
    let mut path = String::from("");

    let is_folder = current_metadata.file_type == Folder;

    while current_metadata.parent != current_metadata.id {
        path = format!("{}/{}", current_metadata.decrypted_name, path);
        current_metadata = files.find(current_metadata.parent)?;
    }

    {
        path = format!("{}/{}", current_metadata.decrypted_name, path);
    }
    // Remove the last forward slash if not a folder.
    if !is_folder {
        path.pop();
    }
    Ok(path)
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}

#[cfg(test)]
mod unit_tests {
    use lockbook_models::file_metadata::FileType;

    use crate::model::repo::RepoSource;
    use crate::model::state::temp_config;
    use crate::pure_functions::files;
    use crate::repo::account_repo;
    use crate::service::path_service::Filter;
    use crate::service::{file_service, path_service, test_utils};
    use crate::CoreError;

    #[test]
    fn create_at_path_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let doc = path_service::create_at_path(config, &format!("{}/document", &account.username))
            .unwrap();

        assert_eq!(doc.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();

        assert_eq!(folder.file_type, FileType::Folder);
    }

    #[test]
    fn create_at_path_in_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();
        let document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();

        assert_eq!(folder.file_type, FileType::Folder);
        assert_eq!(document.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_missing_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();
        let folder =
            path_service::get_by_path(config, &format!("{}/folder", &account.username)).unwrap();

        assert_eq!(folder.file_type, FileType::Folder);
        assert_eq!(document.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_missing_folders() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document = path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
        .unwrap();
        let folder1 =
            path_service::get_by_path(config, &format!("{}/folder", &account.username)).unwrap();
        let folder2 =
            path_service::get_by_path(config, &format!("{}/folder/folder", &account.username))
                .unwrap();

        assert_eq!(folder1.file_type, FileType::Folder);
        assert_eq!(folder2.file_type, FileType::Folder);
        assert_eq!(document.file_type, FileType::Document);
    }

    #[test]
    fn create_at_path_path_contains_empty_file_name() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let result =
            path_service::create_at_path(config, &format!("{}//document", &account.username));

        assert_eq!(result, Err(CoreError::PathContainsEmptyFileName));
    }

    #[test]
    fn create_at_path_path_starts_with_non_root() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let result = path_service::create_at_path(
            config,
            &format!("{}/folder/document", "not-account-username"),
        );

        assert_eq!(result, Err(CoreError::PathStartsWithNonRoot));
    }

    #[test]
    fn create_at_path_path_taken() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
            .unwrap();
        let result =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username));

        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn create_at_path_not_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(config, &format!("{}/not-folder", &account.username)).unwrap();
        let result = path_service::create_at_path(
            config,
            &format!("{}/not-folder/document", &account.username),
        );

        assert_eq!(result, Err(CoreError::FileNotFolder));
    }

    #[test]
    fn get_by_path_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let created_document =
            path_service::create_at_path(config, &format!("{}/document", &account.username))
                .unwrap();
        let document =
            path_service::get_by_path(config, &format!("{}/document", &account.username)).unwrap();

        assert_eq!(created_document, document);
    }

    #[test]
    fn get_by_path_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let created_folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();
        let folder =
            path_service::get_by_path(config, &format!("{}/folder", &account.username)).unwrap();

        assert_eq!(created_folder, folder);
    }

    #[test]
    fn get_by_path_document_in_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let created_document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();
        let document =
            path_service::get_by_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();

        assert_eq!(created_document, document);
    }

    #[test]
    fn get_path_by_id_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document =
            path_service::create_at_path(config, &format!("{}/document", &account.username))
                .unwrap();
        let document_path = path_service::get_path_by_id(config, document.id).unwrap();

        assert_eq!(&document_path, &format!("{}/document", &account.username));
    }

    #[test]
    fn get_path_by_id_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let folder =
            path_service::create_at_path(config, &format!("{}/folder/", &account.username))
                .unwrap();
        let folder_path = path_service::get_path_by_id(config, folder.id).unwrap();

        assert_eq!(&folder_path, &format!("{}/folder/", &account.username));
    }

    #[test]
    fn get_path_by_id_document_in_folder() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        let document =
            path_service::create_at_path(config, &format!("{}/folder/document", &account.username))
                .unwrap();
        let document_path = path_service::get_path_by_id(config, document.id).unwrap();

        assert_eq!(&document_path, &format!("{}/folder/document", &account.username));
    }

    #[test]
    fn get_all_paths() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
        .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
        .unwrap();

        let all_paths = path_service::get_all_paths(config, None).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
        assert_eq!(all_paths.len(), 5);
    }

    #[test]
    fn get_all_paths_documents_only() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
        .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
        .unwrap();

        let all_paths = path_service::get_all_paths(config, Some(Filter::DocumentsOnly)).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
        assert_eq!(all_paths.len(), 1);
    }

    #[test]
    fn get_all_paths_folders_only() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
        .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
        .unwrap();

        let all_paths = path_service::get_all_paths(config, Some(Filter::FoldersOnly)).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
        assert_eq!(all_paths.len(), 4);
    }

    #[test]
    fn get_all_paths_leaf_nodes_only() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Local, &root).unwrap();

        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/document", &account.username),
        )
        .unwrap();
        path_service::create_at_path(
            config,
            &format!("{}/folder/folder/folder/", &account.username),
        )
        .unwrap();

        let all_paths = path_service::get_all_paths(config, Some(Filter::LeafNodesOnly)).unwrap();
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
        assert!(all_paths
            .iter()
            .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
        assert_eq!(all_paths.len(), 2);
    }
}
