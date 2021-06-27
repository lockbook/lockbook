use crate::model::state::Config;
use crate::repo::{file_repo, root_repo};
use crate::service::{file_encryption_service, file_service};
use crate::CoreError;
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};

pub fn create_at_path(config: &Config, path_and_name: &str) -> Result<FileMetadata, CoreError> {
    if path_and_name.contains("//") {
        return Err(CoreError::PathContainsEmptyFileName);
    }

    debug!("Creating path at: {}", path_and_name);
    let path_components = split_path(path_and_name);

    let is_folder = path_and_name.ends_with('/');

    let mut current = root_repo::maybe_get(config)?.ok_or(CoreError::RootNonexistent)?;

    if file_encryption_service::get_name(&config, &current)? != path_components[0] {
        return Err(CoreError::PathStartsWithNonRoot);
    }

    if path_components.len() == 1 {
        return Err(CoreError::PathTaken);
    }

    // We're going to look ahead, and find or create the right child
    'path: for index in 0..path_components.len() - 1 {
        let children = file_repo::get_children(config, current.id)?;

        let next_name = path_components[index + 1];
        debug!("child we're searching for: {}", next_name);

        for child in children {
            if file_encryption_service::get_name(&config, &child)? == next_name {
                // If we're at the end and we find this child, that means this path already exists
                if index == path_components.len() - 2 {
                    return Err(CoreError::PathTaken);
                }

                if child.file_type == Folder {
                    current = child;
                    continue 'path; // Child exists, onto the next one
                }
            }
        }

        // Child does not exist, create it
        let file_type = if is_folder || index != path_components.len() - 2 {
            Folder
        } else {
            Document
        };

        current = file_service::create(config, next_name, current.id, file_type)?;
    }

    Ok(current)
}

pub fn get_by_path(config: &Config, path: &str) -> Result<FileMetadata, CoreError> {
    let root = root_repo::maybe_get(&config)?
        .ok_or_else(|| CoreError::Unexpected(String::from("no root")))?;

    let paths = split_path(path);
    let mut current = root;

    for (i, value) in paths.iter().enumerate() {
        if *value != file_encryption_service::get_name(&config, &current)? {
            return Err(CoreError::FileNonexistent);
        }

        if i + 1 == paths.len() {
            return Ok(current);
        }

        let children = file_repo::get_children(&config, current.id)?;
        let mut found_child = false;

        for child in children {
            let child_name = file_encryption_service::get_name(&config, &child)?;

            if child_name == paths[i + 1] {
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
    let files = file_repo::get_all_metadata(&config)?.union();

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
            let current_name = file_encryption_service::get_name(&config, &current)?;
            if current.file_type == Document {
                current_path = current_name;
            } else {
                current_path = format!("{}/{}", current_name, current_path);
            }
            current = file_repo::get_metadata(&config, current.parent)?.0;
        }

        let root_name = file_encryption_service::get_name(&config, &current)?;
        current_path = format!("{}/{}", root_name, current_path);
        paths.push(current_path.to_string());
    }

    Ok(paths)
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}
